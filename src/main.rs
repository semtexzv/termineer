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

mod gui;
mod mcp;
mod output;
mod prompts;
pub mod serde;
mod tools;
mod tui;
mod version_check;
mod workflow;

use crate::agent::AgentId;
use anyhow::format_err;
use clap::Parser;
use cli::{cli_to_config, Cli, Commands};
use config::Config;
use crossterm::{
    cursor, execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
};
use std::collections::HashMap;
use std::io;
use std::time::Duration;
use tui::TuiInterface;

/// Get comprehensive information about all MCP tools
///
/// This function returns structured information about all available MCP tools,
/// including their names, descriptions, and server associations.
///
/// Returns a map of server names to vectors of (tool_name, description) tuples
pub async fn get_mcp_tools_info() -> HashMap<String, Vec<(String, String)>> {
    // Get the list of provider names using the MCP API
    let provider_names = crate::mcp::get_provider_names();
    let mut result = HashMap::new();

    // For each provider, get all tools and their descriptions
    for server_name in provider_names {
        // Get the provider and list its tools
        if let Some(provider) = crate::mcp::get_provider(&server_name) {
            let tools = provider.list_tools();

            if !tools.is_empty() {
                let mut tool_info = Vec::new();

                for tool in tools {
                    let description = if tool.description.is_empty() {
                        "No description".to_string()
                    } else {
                        tool.description.clone()
                    };

                    tool_info.push((tool.name.clone(), description));
                }

                result.insert(server_name, tool_info);
            }
        }
    }

    result
}

/// Initialize MCP servers and log available methods to the current buffer
///
/// This function handles both initialization and logging in a single operation
/// using the McpManager API.
async fn initialize_and_log_mcp() {
    // Initialize MCP connections from config (silent mode = true)
    if let Err(e) = crate::mcp::config::initialize_mcp_from_config(true).await {
        bprintln!(error: "Failed to initialize MCP connections: {}", e);
        // Continue even if MCP initialization fails
    }

    // Check if we have any providers
    if !crate::mcp::has_providers() {
        // No providers available, nothing to log
        return;
    }

    // Get the list of provider names
    let provider_names = crate::mcp::get_provider_names();

    // Log header
    bprintln!(
        "\n🔌 {}Available MCP tools:{}",
        crate::constants::FORMAT_BOLD,
        crate::constants::FORMAT_RESET
    );

    // Get all tools for all providers
    for provider_name in provider_names {
        // Get tools for this provider using the MCP API
        let tools = crate::mcp::get_provider(&provider_name)
            .unwrap()
            .list_tools();

        if !tools.is_empty() {
            // Log provider name and tool count
            bprintln!(
                "{}📦 {} ({} tools){}",
                crate::constants::FORMAT_BLUE,
                provider_name,
                tools.len(),
                crate::constants::FORMAT_RESET
            );

            // Log each tool with its description
            for tool in tools {
                let description = if tool.description.is_empty() {
                    "No description".to_string()
                } else {
                    tool.description.clone()
                };

                bprintln!(
                    "  • {}{}{}: {}",
                    crate::constants::FORMAT_BOLD,
                    tool.name,
                    crate::constants::FORMAT_RESET,
                    description
                );
            }
        }
    }

    // Add a blank line at the end
    bprintln!("");
}

// MCP initialization is now done directly within each execution context's buffer scope

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

    // Note: MCP servers will now be initialized with a buffer right before agent creation

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
            list_available_kinds().map_err(|e| format_err!("Error listing kinds: {}", e))?;
            return Ok(());
        }
        Some(Commands::Gui) => {
            // Start the GUI
            gui::run_gui();
            return Ok(());
        }
        Some(Commands::Workflow {
            name,
            parameters,
            query,
        }) => {
            // Check if user has Pro access - workflows are a Pro-only feature
            if config::get_app_mode() != config::AppMode::Pro {
                execute!(
                    io::stdout(),
                    SetForegroundColor(Color::Yellow),
                    Print("⚠️ Workflows are a Pro-only feature"),
                    ResetColor,
                    cursor::MoveToNextLine(1),
                )
                .unwrap();
                println!(
                    "Upgrade to Pro for access to workflows and advanced orchestration features."
                );
                return Ok(());
            }

            // Convert the query vector to an Option<String> by joining with spaces
            let query_string = if !query.is_empty() {
                Some(query.join(" "))
            } else {
                None
            };

            // Run in workflow mode
            run_workflow_mode(config, name.clone(), parameters.clone(), query_string)
                .await
                .map_err(|e| format_err!("Error in workflow mode: {}", e))?;

            return Ok(());
        }
        #[cfg(debug_assertions)]
        Some(Commands::DumpPrompts { .. }) => {
            // Dump prompt templates and exit
            // The template name is already in the config
            if let Some(template_name) = &config.dump_prompts {
                // Determine grammar: Use specified grammar or default to XML for dumping
                let grammar_type = config.grammar_type.unwrap_or(
                    // Default to XML if no grammar is specified via --grammar flag
                    crate::prompts::grammar::formats::GrammarType::XmlTags,
                );
                let grammar = crate::prompts::grammar::formats::get_grammar_by_type(grammar_type);

                // Enable all possible tools for dumping
                let mut all_tools_vec: Vec<&str> = crate::prompts::ALL_TOOLS.to_vec();
                all_tools_vec.extend_from_slice(crate::prompts::PLUS_TOOLS);
                all_tools_vec.sort_unstable();
                all_tools_vec.dedup();

                // Render the template
                match prompts::render_template(template_name, &all_tools_vec, grammar) {
                    Ok(rendered_content) => {
                        // Print the rendered template content to stdout
                        println!(
                            "// Template: {} (Grammar: {:?})",
                            template_name, grammar_type
                        );
                        println!("{}", rendered_content);
                    }
                    Err(e) => {
                        // Error during rendering
                        eprintln!("Error rendering template '{}': {}", template_name, e);
                        // List available templates to help the user
                        eprintln!("\nAvailable templates:");
                        for available in prompts::protected::list_available_templates() {
                            eprintln!("  - {}", available);
                        }
                        std::process::exit(1);
                    }
                }
            } else {
                // This case should theoretically not be reachable because clap enforces
                // the `template` argument for the `DumpPrompts` command.
                // However, handle it defensively.
                eprintln!("Error: No template name specified for dump-prompts.");
                std::process::exit(1);
            }
            return Ok(());
        }
        None => {
            // Check if we have a query for non-interactive mode
            if let Some(query) = cli.query {
                // Run in single query mode
                run_single_query_mode(config, query)
                    .await
                    .map_err(|e| format_err!("Error in single query mode: {}", e))?;
            } else {
                // Run in interactive mode
                run_interactive_mode(config)
                    .await
                    .map_err(|e| format_err!("Error in interactive mode: {}", e))?;
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
async fn run_interactive_mode(config: Config) -> anyhow::Result<()> {
    // Check if stdin is a TTY (interactive terminal)
    let is_tty = atty::is(atty::Stream::Stdin);

    if !is_tty {
        // Non-interactive mode requires a TTY for the TUI
        eprintln!("TUI interface requires an interactive terminal. Exiting...");
        return // Explicit use of Result with the expected return type
            Ok(());
    }

    // Create a default buffer to be shared between the main agent and TUI
    let default_buffer = crate::output::SharedBuffer::new(200);

    // Use a single buffer scope for both MCP initialization and agent creation
    let main_agent_id = crate::output::CURRENT_BUFFER
        .scope(default_buffer.clone(), async {
            // Initialize MCP servers and log available methods in a single operation
            initialize_and_log_mcp().await;

            // Create the main agent and capture its ID
            let result: anyhow::Result<AgentId> = match agent::create_agent_with_buffer(
                "main".to_string(),
                config,
                default_buffer.clone(),
            ) {
                Ok(id) => {
                    bprintln!(
                        "🤖 {}Agent{} 'main' created successfully with ID: {}",
                        crate::constants::FORMAT_BOLD,
                        crate::constants::FORMAT_RESET,
                        id
                    );
                    Ok(id)
                }
                Err(e) => {
                    // Use buffer printing for the error
                    bprintln!(error: "Failed to create main agent: {}", e);

                    // Also print to stderr for TUI visibility
                    execute!(
                        io::stderr(),
                        SetForegroundColor(Color::Red),
                        Print(format!("Failed to create main agent: {}", e)),
                        ResetColor,
                        cursor::MoveToNextLine(1)
                    )?;

                    Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to create main agent: {}", e),
                    ))
                    .into())
                }
            };
            result
        })
        .await?;

    output::spawn_with_buffer(default_buffer.clone(), async move {
        match version_check::check_for_updates().await {
            Ok((has_update, _latest_version, message)) => {
                if has_update {
                    if let Some(msg) = message {
                        bprintln!(
                            "\n{}{}{}\n",
                            crate::constants::FORMAT_YELLOW,
                            msg,
                            crate::constants::FORMAT_RESET
                        );
                    };
                }
            }
            Err(e) => bprintln!(error: "Failed to check for updates: {:?}", e),
        }
    });

    // Initialize and run the TUI interface with the same buffer
    let mut tui = TuiInterface::new(main_agent_id)?;
    tui.run().await.unwrap();

    // When TUI exits, terminate all agents
    agent::terminate_all().await;

    // Explicit use of Result with the expected return type
    Ok(())
}

/// Run the application in workflow mode
async fn run_workflow_mode(
    config: Config,
    name: Option<String>,
    parameters: Vec<String>,
    query_string: Option<String>,
) -> anyhow::Result<()> {
    // Create a default buffer for output
    let default_buffer = crate::output::SharedBuffer::new(200);

    crate::output::CURRENT_BUFFER
        .scope(default_buffer.clone(), async {
            // Use a single buffer scope for both MCP initialization and agent creation

            // Initialize MCP servers and log available methods in a single operation
            initialize_and_log_mcp().await;

            // Create the main agent and capture its ID
            let main_agent_id: anyhow::Result<AgentId> = match agent::create_agent_with_buffer(
                "main".to_string(),
                config.clone(),
                default_buffer.clone(),
            ) {
                Ok(id) => {
                    bprintln!(
                        "🤖 {}Agent{} 'main' created successfully with ID: {}",
                        crate::constants::FORMAT_BOLD,
                        crate::constants::FORMAT_RESET,
                        id
                    );
                    Ok(id)
                }
                Err(e) => {
                    bprintln!(error: "Failed to create main agent: {}", e);
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to create main agent: {}", e),
                    )
                    .into())
                }
            };

            // Load the workflow
            match workflow::loader::load_workflow(&name.clone().unwrap_or_default()) {
                Ok(workflow) => {
                    // Parse parameters
                    let mut params = HashMap::new();
                    for param in parameters {
                        if let Some((key, value)) = param.split_once('=') {
                            params.insert(
                                key.to_string(),
                                serde_yaml::Value::String(value.to_string()),
                            );
                        }
                    }

                    // Execute workflow
                    if let Err(e) = workflow::executor::execute_workflow(
                        &workflow,
                        params,
                        query_string.clone(),
                        main_agent_id?,
                    )
                    .await
                    {
                        bprintln!(error: "Workflow error: {}", e);
                    }

                    // Clean up: terminate all agents
                    agent::terminate_all().await;

                    Ok(())
                }
                Err(e) => {
                    bprintln!(error: "Failed to load workflow: {}", e);

                    // Clean up: terminate all agents
                    agent::terminate_all().await;

                    Err(anyhow::anyhow!("Failed to load workflow: {}", e))
                }
            }
        })
        .await
}

/// Run the application in single query mode (non-interactive)
async fn run_single_query_mode(config: Config, query: String) -> anyhow::Result<()> {
    // Extract the timeout value before config is moved
    let timeout_seconds = config.timeout_seconds.unwrap_or(150); // Default to 150 seconds (2.5 minutes) if not specified

    // Set up Ctrl+C handler - use this simplified approach
    ctrlc::set_handler(move || {
        eprintln!("\nOperation interrupted by user");
        std::process::exit(130); // Standard exit code for Ctrl+C termination
    })
    .expect("Failed to set Ctrl+C handler");

    // Create a default buffer for output
    let default_buffer = crate::output::SharedBuffer::new(200);

    // Use a single buffer scope for both MCP initialization and agent creation
    let main_agent_id = crate::output::CURRENT_BUFFER
        .scope(default_buffer.clone(), async {
            // Initialize MCP servers and log available methods in a single operation
            initialize_and_log_mcp().await;

            // Create the main agent and capture its ID
            let result: anyhow::Result<AgentId> = match agent::create_agent_with_buffer(
                "main".to_string(),
                config,
                default_buffer.clone(),
            ) {
                Ok(id) => {
                    bprintln!(
                        "🤖 {}Agent{} 'main' created successfully with ID: {}",
                        crate::constants::FORMAT_BOLD,
                        crate::constants::FORMAT_RESET,
                        id
                    );
                    Ok(id)
                }
                Err(e) => {
                    // Use buffer printing for the error
                    bprintln!(error: "Failed to create main agent: {}", e);

                    // Also print to stderr for CLI visibility
                    eprintln!("Failed to create main agent: {}", e);

                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to create main agent: {}", e),
                    )
                    .into())
                }
            };
            result
        })
        .await?;

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
    // timeout_seconds was extracted at the beginning of the function
    let final_response =
        match agent::run_agent_to_completion(main_agent_id, query, Some(timeout_seconds)).await {
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
