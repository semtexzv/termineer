//! autoswe console interface for AI assistants
//!
//! A command-line interface for interacting with AI assistants through various backends.

use std::env;
use std::io::{self, Write};

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

/// Read a line of input from the user
fn read_line() -> Result<String, Box<dyn std::error::Error>> {
    // Simply use standard readline for simplicity
    let mut input = String::new();

    // Print the prompt
    print!("> ");
    io::stdout().flush()?;

    // Read input
    io::stdin().read_line(&mut input)?;

    // Trim newline
    input = input.trim_end().to_string();

    Ok(input)
}

/// Run in interactive mode with a conversation UI
fn run_interactive_mode(config: Config) -> Result<(), Box<dyn std::error::Error>> {
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
        match session::load_last_session(&mut client) {
            Ok(_) => println!("Successfully resumed last session"),
            Err(e) => println!("Note: Could not resume last session: {}", e),
        }
    }

    // Application header with consistent formatting
    println!("{}AI Assistant Console Interface{}", FORMAT_BOLD, FORMAT_RESET);
    println!("Type your message and press Enter to chat with the assistant");
    println!("Type '/help' for available commands or '/exit' to quit");

    if client.config.enable_tools {
        println!("Tools are ENABLED. The assistant will use tools automatically based on your request.");
    } else {
        println!("Tools are DISABLED. Use /tools on to enable them.");
    }

    // Display token optimization settings
    println!("\nToken optimization settings:");
    println!(
        "- Thinking budget: {} tokens",
        client.config.thinking_budget
    );
    println!(
        "- System prompt: {}",
        if client.config.system_prompt.is_some() {
            "custom"
        } else if client.config.use_minimal_prompt {
            "minimal"
        } else {
            "standard"
        }
    );
    println!();

    loop {
        let user_input = match read_line() {
            Ok(input) => input,
            Err(e) => {
                println!("Input error: {}", e);
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
            if let Err(e) = commands::handle_command(&mut client, user_input) {
                println!("Command error: {}", e);
            }
            continue;
        }

        // Process normal user query
        if !user_input.is_empty() {
            if let Err(e) = process_user_query(&mut client, user_input, false) {
                println!("\nError: {}\n", e);
            }
        }
    }

    println!("Goodbye!");
    Ok(())
}

/// Run a single query in non-interactive mode
fn run_query(config: Config, query: &str) -> Result<(), Box<dyn std::error::Error>> {
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
        match session::load_last_session(&mut client) {
            Ok(_) => println!("Successfully resumed last session"),
            Err(e) => println!("Note: Could not resume last session: {}", e),
        }
    }

    // Process the query
    let result = process_user_query(&mut client, query, false);

    // Print the output of the query
    match &result {
        Ok((_, output)) => {
            if !output.is_empty() {
                println!("{}", output);
            }
        },
        Err(e) => {
            eprintln!("Error processing query: {}", e);
        }
    }

    // Only care about success/error, not the actual result values
    result.map(|_| ())
}

/// Display usage text
fn print_usage() {
    use constants::*;

    // Create usage text using template
    let usage_text = format_template(USAGE_TEMPLATE);

    eprintln!("{}", usage_text);
}

/// Main entry point
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if exists
    let _ = dotenvy::dotenv();

    // Load configuration from environment variables
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: {}", e);
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
        Ok(ArgResult::Query(query)) => run_query(config, &query)?,
        Ok(ArgResult::Interactive) => run_interactive_mode(config)?,
        Ok(ArgResult::ShowHelp) => {
            print_usage();
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }

    Ok(())
}