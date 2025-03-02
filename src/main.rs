//! autoswe console interface for AI assistants
//!
//! A command-line interface for interacting with AI assistants through various backends.

use std::env;
use std::io::{self, Write};
use crossterm::terminal;

mod agent;
mod commands;
mod config;
mod constants;
mod conversation;
pub mod jsonpath;
mod llm;
mod prompts;
pub mod serde_element_array;
mod session;
mod tools;

use agent::{Agent, process_user_query};
use config::{ArgResult, Config};
use prompts::{generate_minimal_system_prompt, ToolDocOptions};

/// Read a line of input from the user using standard terminal input
fn read_line() -> Result<String, Box<dyn std::error::Error>> {
    // Print the prompt
    print!("> ");
    io::stdout().flush()?;
    
    // Use standard readline functionality
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    
    // Trim the trailing newline
    let input = input.trim_end().to_string();
    
    Ok(input)
}

/// Run in interactive mode with a conversation UI
async fn run_interactive_mode(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    use constants::{FORMAT_BOLD, FORMAT_RESET};
    
    let mut client = Agent::new(config.clone());

    // Apply appropriate system prompt if none is provided
    if client.config.system_prompt.is_none() {
        // Use default (full) tool options
        let options = ToolDocOptions::default();

        if client.config.use_minimal_prompt {
            // Use minimal prompt if configured
            let minimal_prompt = generate_minimal_system_prompt(&options);
            client.set_system_prompt(minimal_prompt);
        } else {
            // Use full default system prompt
            let default_prompt = prompts::generate_system_prompt(&options);
            client.set_system_prompt(default_prompt);
        }
    }

    // Attempt to resume last session if requested
    if config.resume_last_session {
        match session::load_last_session(&mut client).await {
            Ok(_) => print!("Successfully resumed last session\r\n"),
            Err(e) => print!("Note: Could not resume last session: {}\r\n", e),
        }
        io::stdout().flush()?;
    }

    // Application header with consistent formatting
    print!("{}AI Assistant Console Interface{}\r\n", FORMAT_BOLD, FORMAT_RESET);
    print!("Type your message and press Enter to chat with the assistant\r\n");
    print!("Type '/help' for available commands or '/exit' to quit\r\n");

    if client.config.enable_tools {
        print!("Tools are ENABLED. The assistant will use tools automatically based on your request.\r\n");
    } else {
        print!("Tools are DISABLED. Use /tools on to enable them.\r\n");
    }

    // Display token optimization settings
    print!("\r\nToken optimization settings:\r\n");
    print!(
        "- Thinking budget: {} tokens\r\n",
        client.config.thinking_budget
    );
    print!(
        "- System prompt: {}\r\n",
        if client.config.system_prompt.is_some() {
            "custom"
        } else if client.config.use_minimal_prompt {
            "minimal"
        } else {
            "standard"
        }
    );
    print!("\r\n");
    io::stdout().flush()?;

    loop {
        let user_input = match read_line() {
            Ok(input) => input,
            Err(e) => {
                print!("Input error: {}\r\n", e);
                io::stdout().flush()?;
                continue;
            }
        };

        let user_input = user_input.trim();

        // Handle exit command directly for quick exit
        if user_input == "/exit" {
            break;
        }

        // Handle commands
        if user_input.starts_with('/') {
            if let Err(e) = commands::handle_command(&mut client, user_input).await {
                print!("Command error: {}\r\n", e);
                io::stdout().flush()?;
            }
            continue;
        }

        // Process normal user query
        if !user_input.is_empty() {
            // Process the query and handle any errors
            // Note: The printing of responses is now handled directly in the agent
            if let Err(e) = process_user_query(&mut client, user_input, false).await {
                print!("\r\nError: {}\r\n\r\n", e);
                io::stdout().flush()?;
            }
        }
    }

    print!("Goodbye!\r\n");
    io::stdout().flush()?;
    
    Ok(())
}

/// Run a single query in non-interactive mode
async fn run_query(config: Config, query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = Agent::new(config.clone());

    // Apply appropriate system prompt if none is provided
    if client.config.system_prompt.is_none() {
        // Use default (full) tool options
        let options = ToolDocOptions::default();

        if client.config.use_minimal_prompt {
            // Use minimal prompt if configured
            let minimal_prompt = generate_minimal_system_prompt(&options);
            client.set_system_prompt(minimal_prompt);
        } else {
            // Use full default system prompt
            let default_prompt = prompts::generate_system_prompt(&options);
            client.set_system_prompt(default_prompt);
        }
    }

    // Attempt to resume last session if requested
    if config.resume_last_session {
        match session::load_last_session(&mut client).await {
            Ok(_) => print!("Successfully resumed last session\r\n"),
            Err(e) => print!("Note: Could not resume last session: {}\r\n", e),
        }
        io::stdout().flush()?;
    }

    // Process the query - printing is now handled in the agent
    let result = process_user_query(&mut client, query, false).await;

    // Only log errors, actual response output is handled in the agent
    if let Err(e) = &result {
        // Use stderr for error messages in non-interactive mode
        eprint!("Error processing query: {}\r\n", e);
        std::io::stderr().flush()?;
    }

    // Only care about success/error, not the actual result values
    result.map(|_| ())
}

/// Display usage text
fn print_usage() {
    use constants::*;

    // Create usage text using template
    let usage_text = format_template(USAGE_TEMPLATE);

    // Use stderr for usage output
    eprint!("{}\r\n", usage_text);
    std::io::stderr().flush().unwrap();
}

/// Main entry point
/// This application uses normal terminal mode, with raw mode only enabled in the shell tool when needed
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if exists
    let _ = dotenvy::dotenv();

    // Load configuration from environment variables
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprint!("Error: {}\r\n", e);
            std::io::stderr().flush()?;
            return Ok(());
        }
    };

    // Apply command line arguments to override configuration
    let args: Vec<String> = env::args().collect();
    let mut config = config; // Make config mutable to apply args

    // Apply args and get result type (query, interactive, or help)
    let arg_result = config.apply_args(&args);

    // Handle the different result types
    match arg_result {
        Ok(ArgResult::Query(query)) => run_query(config, &query).await?,
        Ok(ArgResult::Interactive) => run_interactive_mode(config).await?,
        Ok(ArgResult::ShowHelp) => {
            print_usage();
        }
        Err(e) => {
            eprint!("{}\r\n", e);
            std::io::stderr().flush()?;
        }
    }

    Ok(())
}