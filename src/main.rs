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
pub mod serde;
mod tools;
mod tui;
mod workflow;

use clap::Parser;
use std::io;
use std::time::Duration;
use std::collections::HashMap;
use anyhow::format_err;
use cli::{Cli, Commands, cli_to_config};
use config::Config;
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use tui::TuiInterface;

/// Main entry point for the application
///
/// Sets up the application environment, creates the main agent,
/// and initializes the TUI interface or runs in single query mode.
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load environment variables from .env file
    let _ = dotenvy::dotenv();

    // Parse command line arguments using clap
    let cli = Cli::parse();
    
    // Convert to application config
    let config = cli_to_config(&cli);
    
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
            list_available_kinds().map_err(|e| {
                format_err!("Error listing kinds: {}", e)
            })?;
            return Ok(());
        }
        Some(Commands::Workflow { name, parameters, query }) => {
            // Convert the query vector to an Option<String> by joining with spaces
            let query_string = if !query.is_empty() {
                Some(query.join(" "))
            } else {
                None
            };
            
            // Create the main agent
            let main_agent_id = match agent::create_agent("main".to_string(), config.clone()) {
                Ok(id) => id,
                Err(e) => {
                    eprintln!("Failed to create main agent: {}", e);
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to create main agent: {}", e),
                    ).into());
                }
            };
            
            // Load the workflow
            match workflow::loader::load_workflow(&name.clone().unwrap_or_default()) {
                Ok(workflow) => {
                    // Parse parameters
                    let mut params = HashMap::new();
                    for param in parameters {
                        if let Some((key, value)) = param.split_once('=') {
                            params.insert(key.to_string(), serde_yaml::Value::String(value.to_string()));
                        }
                    }
                    
                    // Execute workflow
                    if let Err(e) = workflow::executor::execute_workflow(&workflow, params, query_string.clone(), main_agent_id).await {
                        eprintln!("Workflow error: {}", e);
                    }
                    
                    // Clean up: terminate all agents
                    agent::terminate_all().await;
                    
                    return Ok(());
                },
                Err(e) => {
                    eprintln!("Failed to load workflow: {}", e);
                    
                    // Clean up: terminate all agents
                    agent::terminate_all().await;
                    
                    return Ok(());
                }
            }
        }
        Some(Commands::DumpPrompts { .. }) => {
            // Dump prompt templates and exit
            // The template name is already in the config
            if let Some(template_name) = &config.dump_prompts {
                match prompts::protected::get_prompt_template(template_name) {
                    Some(template_content) => {
                        // Print the template content to stdout
                        println!("// Template: {}", template_name);
                        println!("{}", template_content);
                    },
                    None => {
                        // Template not found
                        eprintln!("Error: Template '{}' not found", template_name);
                        
                        // List available templates to help the user
                        eprintln!("\nAvailable templates:");
                        for available in prompts::protected::list_available_templates() {
                            eprintln!("  - {}", available);
                        }
                        
                        std::process::exit(1);
                    }
                }
            } else {
                // This shouldn't happen as the command requires a template parameter
                eprintln!("Error: No template name specified");
                std::process::exit(1);
            }
            return Ok(());
        }
        None => {
            // Check if we have a query for non-interactive mode
            if let Some(query) = cli.query {
                // Run in single query mode
                run_single_query_mode(config, query).await.map_err(|e| {
                    format_err!("Error in single query mode: {}", e)
                })?;
            } else {
                // Run in interactive mode
                run_interactive_mode(config).await.map_err(|e| {
                    format_err!("Error in interactive mode: {}", e)
                })?;
            }
        }
    }

    println!("Termineer terminated successfully.");
    // Explicit use of Result with the expected return type
    Ok(())
}

/// List all available agent kinds
fn list_available_kinds() -> anyhow::Result<()> {
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

    // Explicit use of Result with the expected return type
    Ok(())
}

/// Run the application in interactive mode with TUI
async fn run_interactive_mode(
    config: Config,
) -> anyhow::Result<()> {
    // Check if stdin is a TTY (interactive terminal)
    let is_tty = atty::is(atty::Stream::Stdin);

    if !is_tty {
        // Non-interactive mode requires a TTY for the TUI
        eprintln!("TUI interface requires an interactive terminal. Exiting...");
        return // Explicit use of Result with the expected return type
    Ok(());
    }

    // Create the main agent
    let main_agent_id = match agent::create_agent("main".to_string(), config) {
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
            )).into());
        }
    };

    // Initialize and run the TUI interface
    let mut tui = TuiInterface::new(main_agent_id)?;
    tui.run().await.unwrap();

    // When TUI exits, terminate all agents
    agent::terminate_all().await;

    // Explicit use of Result with the expected return type
    Ok(())
}

/// Run the application in single query mode (non-interactive)
async fn run_single_query_mode(
    config: Config,
    query: String,
) -> anyhow::Result<()> {
    // Set up Ctrl+C handler - use this simplified approach
    ctrlc::set_handler(move || {
        eprintln!("\nOperation interrupted by user");
        std::process::exit(130); // Standard exit code for Ctrl+C termination
    })
    .expect("Failed to set Ctrl+C handler");

    // Create the main agent
    let main_agent_id = match agent::create_agent("main".to_string(), config) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to create main agent: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to create main agent: {}", e),
            ).into());
        }
    };

    // Set up buffer streaming for real-time feedback
    let mut last_line_count = 0;
    let mut buffer_check_time = std::time::Instant::now();
    
    // Spawn a task to continuously stream buffer content to stderr
    let buffer_task = tokio::spawn(async move {
        loop {
            // Check every 100ms
            if buffer_check_time.elapsed() >= Duration::from_millis(100) {
                buffer_check_time = std::time::Instant::now();
                
                // Get the current buffer content
                if let Ok(buffer) = agent::get_agent_buffer(main_agent_id) {
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
            
            // Sleep briefly to avoid tight loop
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });

    eprintln!("Processing query, please wait...");

    // Run the agent and wait for completion
    let timeout_seconds = 150; // 2.5 minutes timeout
    let final_response = match agent::run_agent_to_completion(
        main_agent_id,
        query,
        Some(timeout_seconds)
    ).await {
        Ok(response) => response,
        Err(e) => {
            eprintln!("Failed to get response: {}", e);
            String::new()
        }
    };

    // Abort the buffer task
    buffer_task.abort();
    
    // Clean up: terminate all agents
    agent::terminate_all().await;

    // Output the final response to stdout
    if final_response.trim().is_empty() {
        println!("No response was generated. Please try again.");
    } else {
        // Just print the raw response without any markup
        println!("{}", final_response.trim());
    }

    // Explicit use of Result with the expected return type
    Ok(())
}