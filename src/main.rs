//! Termineer - AI Agent Console Interface
//!
//! This application provides a command-line interface for interacting with AI agents,
//! supporting multiple agents, tool execution, and conversation management.

#[macro_use]
mod macros;

mod agent;
mod ansi_converter;
mod config;
mod constants;
mod conversation;
pub mod jsonpath;
mod llm;

mod mcp;
mod output;
mod prompts;
pub mod serde_utils;
mod server_auth;
// Session module temporarily disabled until needed
// mod session;
mod tools;
mod ui_interface;

use lazy_static::lazy_static;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{AgentManager, AgentMessage, AgentState};
use config::{ArgResult, Config};
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use tokio::time::sleep;
use ui_interface::TuiInterface;

// Global agent manager available to all components
lazy_static! {
    pub static ref GLOBAL_AGENT_MANAGER: Arc<Mutex<AgentManager>> =
        Arc::new(Mutex::new(AgentManager::new()));
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
    let arg_result = config.apply_args(&args).unwrap();

    // Handle different argument outcomes
    match arg_result {
        ArgResult::Login => {
            // Only attempt authentication for explicit login command
            match authenticate_user(&mut config).await {
                Ok(_) => {
                    // Authentication successful
                    execute!(
                        io::stdout(),
                        SetForegroundColor(Color::Green),
                        Print(format!(
                            "✅ Successfully logged in - You now have {} access",
                            config.app_mode
                        )),
                        ResetColor,
                        cursor::MoveToNextLine(1),
                    )
                    .unwrap();
                    println!("Your login has been saved and will be used for future sessions.");
                    return Ok(());
                }
                Err(e) => {
                    // Authentication failed
                    execute!(
                        io::stderr(),
                        SetForegroundColor(Color::Red),
                        Print(format!("❌ Login failed: {}", e)),
                        ResetColor,
                        cursor::MoveToNextLine(1),
                    )
                    .unwrap();
                    return Err(e);
                }
            }
        }
        ArgResult::ShowHelp => {
            // Display help information and exit
            print_help();
            return Ok(());
        }
        ArgResult::DumpPrompts => {
            // Dump prompt templates and exit
            // TODO: Unimplemented
            return Ok(());
        }
        ArgResult::ListKinds => {
            // List available agent kinds and exit
            list_available_kinds()?;
            return Ok(());
        }
        ArgResult::Query(query) => {
            // For normal usage, default to free mode until user logs in
            if config.skip_auth {
                // Skip authentication entirely if requested (dev mode)
            } else {
                // Check if we have cached authentication from previous login
                if let Err(_) = attempt_cached_auth(&mut config).await {
                    // No valid cached auth, use free mode
                    config::set_app_mode(config::AppMode::Free);
                    config.app_mode = config::AppMode::Free;

                    // Show free mode banner
                    execute!(
                        io::stdout(),
                        SetForegroundColor(Color::Blue),
                        Print("🆓 Running in FREE mode - Some features are restricted"),
                        ResetColor,
                        cursor::MoveToNextLine(1),
                    )
                    .unwrap();

                    // Only show this hint occasionally (roughly 20% of the time)
                    if rand::random::<bool>() && rand::random::<bool>() {
                        println!("Tip: Run 'termineer login' to unlock premium features");
                    }
                }
            }

            // Run in single query mode
            run_single_query_mode(agent_manager, config, query).await?;
            return Ok(());
        }
        ArgResult::Interactive => {
            // For normal usage, default to free mode until user logs in
            if config.skip_auth {
                // Skip authentication entirely if requested (dev mode)
            } else {
                // Check if we have cached authentication from previous login
                if let Err(_) = attempt_cached_auth(&mut config).await {
                    // No valid cached auth, use free mode
                    config::set_app_mode(config::AppMode::Free);
                    config.app_mode = config::AppMode::Free;

                    // Show free mode banner
                    execute!(
                        io::stdout(),
                        SetForegroundColor(Color::Blue),
                        Print("🆓 Running in FREE mode - Some features are restricted"),
                        ResetColor,
                        cursor::MoveToNextLine(1),
                    )
                    .unwrap();

                    // Only show this hint occasionally (roughly 33% of the time)
                    if rand::random::<bool>() {
                        println!("Tip: Run 'termineer login' to unlock premium features");
                    }
                }
            }

            // Continue to interactive mode
            run_interactive_mode(agent_manager, config).await?;
        }
    }

    println!("Termineer terminated successfully.");
    Ok(())
}

/// Try to load authentication from a previous session without user interaction
async fn attempt_cached_auth(
    config: &mut Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Use the server_auth module to attempt cached authentication
    use server_auth::{get_app_mode_from_subscription, is_subscription_expired, AuthClient};

    // Try to load saved credentials (this will need to be implemented in server_auth)
    let auth_client = AuthClient::new("https://autoswe-server.fly.dev".to_string());

    // Try to get user info from saved credentials
    match auth_client.get_cached_user_info().await {
        Ok(user_info) => {
            // Check if subscription is expired
            if is_subscription_expired(&user_info) {
                return Err("Your subscription has expired. Please log in again.".into());
            }

            // Authentication successful, set user information
            config.user_email = Some(user_info.email.clone());
            if let Some(subscription) = user_info.subscription_type.clone() {
                config.subscription_type = Some(subscription);
            }

            // Set app mode based on subscription type
            let app_mode = get_app_mode_from_subscription(user_info.subscription_type.as_deref());

            // Update both the config and global app mode
            config::set_app_mode(app_mode.clone());
            config.app_mode = app_mode;

            // Quietly indicate the mode (no big banners)
            println!(
                "✓ Authenticated as {} ({})",
                user_info.email,
                config::get_app_mode()
            );

            Ok(())
        }
        Err(e) => {
            // No valid cached credentials found
            Err(format!("No valid saved credentials: {}", e).into())
        }
    }
}

/// Display help information
fn print_help() {
    let help_text = obfstr::obfstring!(
        r#"Termineer - Your Terminal Engineer

Usage: Termineer [OPTIONS] [QUERY]

If QUERY is provided, runs in non-interactive mode and outputs only the response.
If QUERY is not provided, starts an interactive console session.

Options:
  --model MODEL_NAME     Specify the model to use
                         (default: claude-3-7-sonnet-20250219)
  --grammar TYPE         Specify the grammar type to use (xml, markdown)
                         (default: xml for most models, markdown for Gemini)
  --system TEMPLATE      Specify which template to use (basic, minimal, researcher)
  --no-tools             Disable tools
  --thinking-budget N    Set the thinking budget in tokens
  --help                 Display this help message

Environment Variables:
  ANTHROPIC_API_KEY      Your Anthropic API key (required for Claude models)
  GOOGLE_API_KEY         Your Google API key (required for Gemini models)

Subscription Tiers:
  Free Mode              Available without authentication, limited to smaller models
  Plus/Pro               Requires authentication, provides access to advanced models

Example:
  Termineer --model claude-3-haiku-20240307 "What is the capital of France?"
  Termineer --model google/gemini-1.5-flash "Explain quantum computing.""#
    );
    println!("{}", help_text);
}

/// List all available agent kinds
fn list_available_kinds() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Print the list of available kinds
    println!("{}", prompts::get_available_kinds());

    // Usage information in a single string
    let usage_text = obfstr::obfstring!(
        r#"
Use with: --kind KIND_NAME
Example: --kind researcher
For advanced templates: --kind plus/researcher"#
    );

    println!("{}", usage_text);

    Ok(())
}

/// Authenticate user with the server using OAuth
///
/// This function connects to the Termineer server to authenticate the user
/// using an OAuth flow that opens the browser for authentication.
async fn authenticate_user(
    config: &mut Config,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Use the real implementation for OAuth authentication
    use crossterm::{
        cursor::MoveToNextLine,
        execute,
        style::{Color, ResetColor, SetForegroundColor},
    };
    use server_auth::{is_subscription_expired, AuthClient};
    use std::io::stdout;

    // Initialize auth client
    let auth_client = AuthClient::new("https://autoswe-server.fly.dev".to_string());

    // Start OAuth flow
    println!(
        "{}",
        obfstr::obfstring!(
            r#"Starting authentication flow...
This will open your browser to authenticate with your account.
If you don't have an account, you can create one during this process."#
        )
    );

    // Perform OAuth authentication
    let user_info = match auth_client.authenticate().await {
        Ok(info) => info,
        Err(e) => {
            // Print error in red
            execute!(stdout(), SetForegroundColor(Color::Red), MoveToNextLine(1),).ok();
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
        )
        .ok();
        println!("⚠️ Your subscription has expired. Please renew your subscription.");
        execute!(stdout(), ResetColor).ok();

        return Err("Your subscription has expired. Please renew your subscription.".into());
    }

    // Log successful authentication with green text
    execute!(
        stdout(),
        SetForegroundColor(Color::Green),
        MoveToNextLine(1),
    )
    .ok();
    println!("✅ Authentication successful for: {}", user_info.email);

    if let Some(subscription) = &user_info.subscription_type {
        println!("📋 Subscription: {}", subscription);
    }

    execute!(stdout(), ResetColor).ok();

    // Save user information for future use
    config.user_email = Some(user_info.email.clone());
    if let Some(subscription) = user_info.subscription_type.clone() {
        config.subscription_type = Some(subscription.clone());
    }

    // Set app mode based on subscription type using our helper function
    let app_mode =
        server_auth::get_app_mode_from_subscription(user_info.subscription_type.as_deref());

    // Update both the config and global app mode
    config::set_app_mode(app_mode.clone());
    config.app_mode = app_mode;

    // Display the mode
    println!("🔑 Access Level: {}", config::get_app_mode());

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
    })
    .expect("Failed to set Ctrl+C handler");

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
        manager.send_message(main_agent_id, AgentMessage::UserInput(query.clone()))?;
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
            manager
                .get_agent_state(main_agent_id)
                .unwrap_or(AgentState::Idle)
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
        eprintln!(
            "Warning: Processing timed out after {} seconds",
            max_wait_time.as_secs()
        );
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
                if content.starts_with("🤖")
                    || content.starts_with("✅")
                    || content.contains("Token usage:")
                    || content.contains(" in / ")
                    || skip_next_line
                {
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
                    if content.starts_with("[")
                        && content.contains(" in / ")
                        && content.contains(" out]")
                    {
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
