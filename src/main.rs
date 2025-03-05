//! AutoSWE - AI Agent Console Interface
//!
//! This application provides a command-line interface for interacting with AI agents,
//! supporting multiple agents, tool execution, and conversation management.

mod agent;
mod ansi_converter;
mod config;
mod constants;
mod conversation;
pub mod jsonpath;
mod llm;
mod macros;
mod mcp;
mod output;
mod prompts;
pub mod serde_utils;
mod server_auth;
// Session module temporarily disabled until needed
// mod session;
mod tools;
mod ui_interface;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use lazy_static::lazy_static;

use agent::{AgentManager, AgentMessage, AgentState};
use config::{Config, ArgResult};
use crossterm::{
    cursor,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use tokio::time::sleep;
use ui_interface::TuiInterface;

// Global agent manager available to all components
lazy_static! {
    pub static ref GLOBAL_AGENT_MANAGER: Arc<Mutex<AgentManager>> = Arc::new(Mutex::new(AgentManager::new()));
}

/// Main entry point for the application
///
/// Sets up the application environment, creates the main agent,
/// and initializes the TUI interface or runs in single query mode.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables from .env file
    let _ = dotenvy::dotenv();

    // Get command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    // Use a reference to the global agent manager
    let agent_manager = GLOBAL_AGENT_MANAGER.clone();

    // Initialize configuration
    let mut config = Config::new();
    
    // Process command line arguments
    let arg_result = config.apply_args(&args)?;
    
    // Skip authentication for specific commands
    let skip_verification = matches!(arg_result, 
        ArgResult::ShowHelp | ArgResult::DumpPrompts | ArgResult::ListKinds);
        
    // Authenticate user if needed
    if !skip_verification && !config.skip_auth {
        if let Err(e) = authenticate_user(&mut config).await {
            execute!(
                io::stderr(),
                SetForegroundColor(Color::Red),
                Print(format!("Authentication failed: {}", e)),
                ResetColor,
                cursor::MoveToNextLine(1),
            )?;
            return Err(e);
        }
    }
    
    match arg_result {
        ArgResult::ShowHelp => {
            // Display help information and exit
            print_help();
            return Ok(());
        },
        ArgResult::DumpPrompts => {
            // Dump prompt templates and exit
            dump_prompt_templates(&config)?;
            return Ok(());
        },
        ArgResult::ListKinds => {
            // List available agent kinds and exit
            list_available_kinds()?;
            return Ok(());
        },
        ArgResult::Query(query) => {
            // Run in single query mode
            run_single_query_mode(agent_manager, config, query).await?;
            return Ok(());
        },
        ArgResult::Interactive => {
            // Continue to interactive mode
            run_interactive_mode(agent_manager, config).await?;
        }
    }
    
    println!("AutoSWE terminated successfully.");
    Ok(())
}

/// Display help information
fn print_help() {
    println!("AutoSWE - Multi-LLM Console Interface");
    println!();
    println!("Usage: AutoSWE [OPTIONS] [QUERY]");
    println!();
    println!("If QUERY is provided, runs in non-interactive mode and outputs only the response.");
    println!("If QUERY is not provided, starts an interactive console session.");
    println!();
    println!("Options:");
    println!("  --model MODEL_NAME     Specify the model to use");
    println!("                         (default: claude-3-7-sonnet-20250219)");
    println!("  --grammar TYPE         Specify the grammar type to use (xml, markdown)");
    println!("                         (default: xml for most models, markdown for Gemini)");
    println!("  --system TEMPLATE      Specify which template to use (basic, minimal, researcher)");
    println!("  --no-tools             Disable tools");
    println!("  --thinking-budget N    Set the thinking budget in tokens");
    println!("  --minimal-prompt       Use a minimal system prompt");
    println!("  --server-url URL       Specify the server URL for authentication");
    println!("  --skip-auth            Skip authentication (for development)");
    println!("  --help                 Display this help message");
    println!();
    println!("Environment Variables:");
    println!("  ANTHROPIC_API_KEY      Your Anthropic API key (required for Claude models)");
    println!("  GOOGLE_API_KEY         Your Google API key (required for Gemini models)");
    println!();
    println!("Example:");
    println!("  AutoSWE --model claude-3-haiku-20240307 \"What is the capital of France?\"");
    println!("  AutoSWE --model google/gemini-1.5-flash \"Explain quantum computing.\"");
}

/// List all available agent kinds
fn list_available_kinds() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    
    // Print the list of available kinds
    println!("{}", prompts::get_available_kinds());
    
    println!();
    println!("Use with: --kind KIND_NAME");
    println!("Example: --kind researcher");
    println!("For advanced templates: --kind plus/researcher");
    
    Ok(())
}

/// Dump prompt templates to stdout
///
/// This function renders and outputs the specified prompt template to stdout.
/// It's used with the --dump-prompts command line flag for debugging purposes.
fn dump_prompt_templates(config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let template_name = match config.dump_prompts.as_deref() {
        Some(name) => name,
        None => {
            return Err("No template name specified for --dump-prompts".into());
        }
    };

    // Convert template name to lowercase for case-insensitive matching
    let template_name = template_name.to_lowercase();
    
    // Get the appropriate list of tools based on config
    let enabled_tools = if config.enable_tools {
        prompts::ALL_TOOLS
    } else {
        prompts::READONLY_TOOLS
    };
    
    // Get the grammar based on config
    let grammar = prompts::select_grammar_by_type(config.grammar_type);
    
    // Render the template
    let prompt_result = match template_name.as_str() {
        "basic" => prompts::render_template("basic", enabled_tools, grammar),
        "minimal" => prompts::render_template("minimal", enabled_tools, grammar),
        "researcher" => prompts::render_template("researcher", enabled_tools, grammar),
        _ => {
            return Err(format!("Unknown template name: {}. Available templates: basic, minimal, researcher", template_name).into());
        }
    };
    
    // Output the rendered template
    match prompt_result {
        Ok(prompt) => println!("{}", prompt),
        Err(e) => return Err(format!("Error rendering template: {}", e).into()),
    }
    
    Ok(())
}

/// Authenticate user with the server using OAuth
///
/// This function connects to the AutoSWE server to authenticate the user
/// using an OAuth flow that opens the browser for authentication.
async fn authenticate_user(config: &mut Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use server_auth::{AuthClient, is_subscription_expired};
    use crossterm::{
        style::{Color, SetForegroundColor, ResetColor},
        execute,
        cursor::MoveToNextLine,
    };
    use std::io::stdout;
    
    // Ensure we have a server URL
    let server_url = match &config.server_url {
        Some(url) => url.clone(),
        None => {
            return Err("Error: Server URL not configured. Please set AUTOSWE_SERVER_URL environment variable.".into());
        }
    };
    
    // Initialize auth client
    let auth_client = AuthClient::new(server_url);
    
    // Start OAuth flow
    println!("Starting authentication flow...");
    println!("This will open your browser to authenticate with your account.");
    println!("If you don't have an account, you can create one during this process.");
    
    // Perform OAuth authentication
    let user_info = match auth_client.authenticate().await {
        Ok(info) => info,
        Err(e) => {
            // Print error in red
            execute!(
                stdout(),
                SetForegroundColor(Color::Red),
                MoveToNextLine(1),
            ).ok();
            println!("❌ Authentication failed: {}", e);
            execute!(stdout(), ResetColor).ok();
            
            return Err(format!("Authentication error: {}", e).into());
        }
    };
    
    // Check if subscription is expired
    if is_subscription_expired(&user_info) {
        // Print error in yellow
        execute!(
            stdout(),
            SetForegroundColor(Color::Yellow),
            MoveToNextLine(1),
        ).ok();
        println!("⚠️ Your subscription has expired. Please renew your subscription.");
        execute!(stdout(), ResetColor).ok();
        
        return Err("Your subscription has expired. Please renew your subscription.".into());
    }
    
    // Log successful authentication with green text
    execute!(
        stdout(),
        SetForegroundColor(Color::Green),
        MoveToNextLine(1),
    ).ok();
    println!("✅ Authentication successful for: {}", user_info.email);
    
    if let Some(subscription) = &user_info.subscription_type {
        println!("📋 Subscription: {}", subscription);
    }
    
    execute!(stdout(), ResetColor).ok();
    
    // Save user information for future use
    config.user_email = Some(user_info.email.clone());
    if let Some(subscription) = user_info.subscription_type.clone() {
        config.subscription_type = Some(subscription);
    }
    
    // Optional: Add a small delay to ensure the user sees the verification message
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    Ok(())
}

/// Run the application in interactive mode with TUI
async fn run_interactive_mode(
    agent_manager: Arc<Mutex<AgentManager>>,
    config: Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if stdin is a TTY (interactive terminal)
    let is_tty = atty::is(atty::Stream::Stdin);

    if !is_tty {
        // Non-interactive mode requires a TTY for the TUI
        eprintln!("TUI interface requires an interactive terminal. Exiting...");
        return Ok(());
    }
    
    // Create the main agent
    let main_agent_id = {
        let mut manager = agent_manager.lock().unwrap();
        
        // Create the main agent
        match manager.create_agent("main".to_string(), config) {
            Ok(id) => id,
            Err(e) => {
                execute!(
                    io::stderr(),
                    SetForegroundColor(Color::Red),
                    Print(format!("Failed to create main agent: {}", e)),
                    ResetColor,
                    cursor::MoveToNextLine(1)
                )?;
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create main agent: {}", e),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
        }
    };

    // Initialize and run the TUI interface
    let mut tui = TuiInterface::new(agent_manager.clone(), main_agent_id)?;
    tui.run().await.unwrap();

    // When TUI exits, terminate all agents
    {
        let mut manager = agent_manager.lock().unwrap();
        manager.terminate_all().await;
    }
    
    Ok(())
}

/// Run the application in single query mode (non-interactive)
async fn run_single_query_mode(
    agent_manager: Arc<Mutex<AgentManager>>,
    config: Config,
    query: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set up Ctrl+C handler - use this simplified approach
    ctrlc::set_handler(move || {
        eprintln!("\nOperation interrupted by user");
        std::process::exit(130); // Standard exit code for Ctrl+C termination
    }).expect("Failed to set Ctrl+C handler");
    
    // Create the main agent
    let main_agent_id = {
        let mut manager = agent_manager.lock().unwrap();
        
        // Create the main agent
        match manager.create_agent("main".to_string(), config) {
            Ok(id) => id,
            Err(e) => {
                eprintln!("Failed to create main agent: {}", e);
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create main agent: {}", e),
                ))
                    as Box<dyn std::error::Error + Send + Sync>);
            }
        }
    };
    
    // Send the query to the agent
    {
        let manager = agent_manager.lock().unwrap();
        manager.send_message(
            main_agent_id,
            AgentMessage::UserInput(query.clone()),
        )?;
    }
    
    // Wait for the agent to complete
    let max_wait_time = Duration::from_secs(150); // 2.5 minutes maximum wait time
    let start_time = std::time::Instant::now();
    
    eprintln!("Processing query, please wait...");
    
    // Use a traditional event loop to check agent state
    let mut final_response = String::new();
    let mut agent_done = false;
    
    // Track lines we've already output to stderr
    let mut last_line_count = 0;
    
    while !agent_done && start_time.elapsed() < max_wait_time {
        // Check agent state
        let state = {
            let manager = agent_manager.lock().unwrap();
            manager.get_agent_state(main_agent_id).unwrap_or(AgentState::Idle)
        };
        
        // Stream new buffer content to stderr
        {
            let manager = agent_manager.lock().unwrap();
            if let Ok(buffer) = manager.get_agent_buffer(main_agent_id) {
                let lines = buffer.lines();
                let current_count = lines.len();
                
                // If there are new lines, print them to stderr
                if current_count > last_line_count {
                    for i in last_line_count..current_count {
                        if let Some(line) = lines.get(i) {
                            eprintln!("{}", line.content);
                        }
                    }
                    last_line_count = current_count;
                }
            }
        }
        
        // Only consider the agent done when we have an explicit Done state with a response
        if let AgentState::Done(Some(response)) = state {
            final_response = response;
            agent_done = true;
            continue;
        }
        
        // If not done, wait briefly to avoid busy-waiting
        if !agent_done {
            sleep(Duration::from_millis(100)).await;
        }
    }
    
    if !agent_done {
        eprintln!("Warning: Processing timed out after {} seconds", max_wait_time.as_secs());
    }
    
    // If we don't have a response from the Done state, extract it from buffer
    if final_response.is_empty() {
        let manager = agent_manager.lock().unwrap();
        
        if let Ok(buffer) = manager.get_agent_buffer(main_agent_id) {
            // Get all the buffer lines
            let lines = buffer.lines();
            
            // Use a state machine to extract the actual assistant response
            let mut found_user_input = false;
            let mut skip_next_line = false;
            
            for line in lines.iter() {
                let content = &line.content;
                
                // Skip processing lines (these are just debugging info)
                if content.starts_with("🤖") || 
                   content.starts_with("✅") ||
                   content.contains("Token usage:") ||
                   content.contains(" in / ") ||
                   skip_next_line {
                    skip_next_line = false;
                    continue;
                }
                
                // Detect the user's query (marked with >)
                if content.starts_with(">") {
                    found_user_input = true;
                    continue;
                }
                
                // Once we've found the user query, start collecting the response
                if found_user_input && !content.trim().is_empty() {
                    // If the line has exactly this format, skip it and the next line
                    if content.starts_with("[") && content.contains(" in / ") && content.contains(" out]") {
                        skip_next_line = true;
                        continue;
                    }
                    
                    final_response.push_str(content);
                    final_response.push('\n');
                }
            }
        }
    }
    
    // Clean up: terminate all agents
    {
        let mut manager = agent_manager.lock().unwrap();
        manager.terminate_all().await;
    }
    
    // Output the final response to stdout
    if final_response.trim().is_empty() {
        println!("No response was generated. Please try again.");
    } else {
        // Just print the raw response without any markup
        println!("{}", final_response.trim());
    }
    
    Ok(())
}
