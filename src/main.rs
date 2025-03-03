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
mod output;
mod prompts;
pub mod serde_element_array;
// Session module temporarily disabled until needed
// mod session;
mod tools;
mod ui_interface;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{AgentManager, AgentMessage, AgentState};
use config::{Config, ArgResult};
use crossterm::{
    cursor,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{enable_raw_mode, disable_raw_mode},
};
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::sleep;
use ui_interface::TuiInterface;

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
    
    // Create the agent manager
    let agent_manager = Arc::new(Mutex::new(AgentManager::new()));

    // Initialize configuration
    let mut config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            execute!(
                io::stderr(),
                SetForegroundColor(Color::Red),
                Print(format!("Error loading configuration: {}", e)),
                ResetColor,
                cursor::MoveToNextLine(1),
            )?;
            Config::new()
        }
    };
    
    // Process command line arguments
    let arg_result = config.apply_args(&args)?;
    
    match arg_result {
        ArgResult::ShowHelp => {
            // Display help information and exit
            print_help();
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
    println!("  --system PROMPT        Provide a system prompt");
    println!("  --no-tools             Disable tools");
    println!("  --thinking-budget N    Set the thinking budget in tokens");
    println!("  --minimal-prompt       Use a minimal system prompt");
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
    
    while !agent_done && start_time.elapsed() < max_wait_time {
        // Check agent state
        let state = {
            let manager = agent_manager.lock().unwrap();
            manager.get_agent_state(main_agent_id).unwrap_or(AgentState::Idle)
        };
        
        // Check if we have a final response from a Done state
        if let AgentState::Done(Some(response)) = state {
            final_response = response;
            agent_done = true;
            continue;
        }
        
        // If the agent is idle but has content, it might be done
        if matches!(state, AgentState::Idle) {
            // Check if we've processed enough to consider ourselves done
            let buffer_has_content = {
                let manager = agent_manager.lock().unwrap();
                if let Ok(buffer) = manager.get_agent_buffer(main_agent_id) {
                    let lines = buffer.lines();
                    // We need at least one line that's not the intro message
                    lines.len() > 1 && lines.iter().any(|line| !line.content.starts_with("🤖"))
                } else {
                    false
                }
            };
            
            // If we have content and have been idle for a while, assume we're done
            if buffer_has_content && start_time.elapsed() > Duration::from_secs(10) {
                // We'll need to extract the response from the buffer later
                agent_done = true;
            }
        }
        
        // If not done, wait briefly to avoid busy-waiting
        if !agent_done {
            sleep(Duration::from_millis(500)).await;
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
