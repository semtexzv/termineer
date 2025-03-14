//! Termineer - AI Agent Console Interface
//!
//! This application provides a command-line interface for interacting with AI agents,
//! supporting multiple agents, tool execution, and conversation management.

#[macro_use]
mod macros;

mod agent;
mod ansi_converter;
mod cli;
mod config;
mod constants;
mod conversation;
pub mod jsonpath;
mod llm;

mod mcp;
mod output;
mod prompts;
pub mod serde_utils;
// Session module temporarily disabled until needed
// mod session;
mod tools;
mod tui;

use clap::Parser;
use lazy_static::lazy_static;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent::{AgentManager, AgentMessage, AgentState};
use cli::{Cli, Commands, cli_to_config};
use config::Config;
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use tokio::time::sleep;
use tui::TuiInterface;

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

    // Parse command line arguments using clap
    let cli = Cli::parse();
    
    // Convert to application config
    let config = cli_to_config(&cli);
    
    // Use a reference to the global agent manager
    let agent_manager = GLOBAL_AGENT_MANAGER.clone();
    
    // Set the app mode based on build configuration
    #[cfg(debug_assertions)]
    {
        // In debug builds, always use Pro mode
        config::set_app_mode(config::AppMode::Pro);
    }
    #[cfg(not(debug_assertions))]
    {
        // In release builds, use Free mode (authentication removed)
        config::set_app_mode(config::AppMode::Free);
    }

    // Handle different command/argument combinations
    match &cli.command {
        Some(Commands::Login) => {
            // Authentication has been removed
            execute!(
                io::stdout(),
                SetForegroundColor(Color::Blue),
                Print("ℹ️ Authentication has been removed from this version"),
                ResetColor,
                cursor::MoveToNextLine(1),
            )
            .unwrap();
            println!("All functionality is available without authentication.");
            return Ok(());
        }
        Some(Commands::ListKinds) => {
            // List available agent kinds and exit
            list_available_kinds()?;
            return Ok(());
        }
        Some(Commands::DumpPrompts { .. }) => {
            // Dump prompt templates and exit
            // Note: The template name is already in the config
            // TODO: Implement prompt dumping functionality
            return Ok(());
        }
        None => {
            // Check if we have a query for non-interactive mode
            if let Some(query) = cli.query {
                // Run in single query mode
                run_single_query_mode(agent_manager, config, query).await?;
            } else {
                // Run in interactive mode
                run_interactive_mode(agent_manager, config).await?;
            }
        }
    }

    println!("Termineer terminated successfully.");
    Ok(())
}

/// List all available agent kinds
fn list_available_kinds() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Print the list of available kinds - use Pro mode to show all kinds for upselling
    println!("{}", prompts::get_kinds_for_mode(config::AppMode::Pro));

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