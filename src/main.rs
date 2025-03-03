//! AutoSWE - AI Agent Console Interface
//!
//! This application provides a command-line interface for interacting with AI agents,
//! supporting multiple agents, tool execution, and conversation management.

mod agent;
mod ansi_converter;
mod config;
mod constants;
mod conversation;
mod conversation_truncation;
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

use agent::AgentManager;
use agent::AgentId;
use config::Config;
use crossterm::{
    cursor,
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use ui_interface::TuiInterface;

/// Main entry point for the application
///
/// Sets up the application environment, creates the main agent,
/// and initializes the TUI interface.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables from .env file
    let _ = dotenvy::dotenv();

    // Create the agent manager
    let agent_manager = Arc::new(Mutex::new(AgentManager::new()));

    // Create the main agent
    let main_agent_id = {
        let mut manager = agent_manager.lock().unwrap();

        // Get configuration from environment or use defaults
        let config = match Config::from_env() {
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

    // Check if stdin is a TTY (interactive terminal) or pipe
    let is_tty = atty::is(atty::Stream::Stdin);

    if !is_tty {
        // Non-interactive mode not supported with TUI
        println!("TUI interface requires an interactive terminal. Exiting...");
        return Ok(());
    }

    // Initialize and run the TUI interface
    let mut tui = TuiInterface::new(agent_manager.clone(), main_agent_id)?;
    tui.run().await.unwrap();

    // When TUI exits, terminate all agents
    {
        let mut manager = agent_manager.lock().unwrap();
        manager.terminate_all().await;
    }
    
    println!("AutoSWE terminated successfully.");
    Ok(())
}
