//! Command processing for the Terminal UI

use crate::agent::{AgentId, AgentMessage};
use crate::agent::types::AgentCommand;
use crate::tui::state::TuiState;
use std::error::Error;

/// Process slash commands
pub async fn process_command(
    state: &mut TuiState,
    input: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Split command and arguments
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let command = parts[0].trim_start_matches('/');
    let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    // Handle different commands
    match command {
        "help" => {
            // Show help information
            let help_text = obfstr::obfstring!(
                "\
            Available commands:
            /help - Show this help information
            /exit, /quit - Exit the application
            /interrupt - Interrupt the current agent
            /model MODEL - Set the model (e.g., claude-3-haiku-20240307)
            /tools on|off - Enable or disable tools
            /system TEXT - Set the system prompt
            /reset - Reset the conversation
            /thinking NUMBER - Set thinking budget in tokens (e.g., 10000)

            Agent selection:
            #ID or #NAME - Switch to agent by ID or name
            "
            );

            show_command_result(state, "Help".to_string(), help_text);
        }

        "exit" | "quit" => {
            // Exit the application
            state.should_quit = true;
        }

        "interrupt" => {
            // Interrupt the current agent
            let manager = state.agent_manager.lock().unwrap();
            manager.interrupt_agent_with_reason(
                state.selected_agent_id,
                "User requested interruption via /interrupt command".to_string(),
            )?;
        }

        "model" => {
            if args.is_empty() {
                show_command_result(
                    state,
                    "Error".to_string(),
                    "Model name is required".to_string(),
                );
                return Ok(());
            }

            // Create SetModel command
            let cmd = AgentCommand::SetModel(args.to_string());

            // Send to agent
            let manager = state.agent_manager.lock().unwrap();
            manager.send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
        }

        "tools" => {
            let enable = match args.to_lowercase().as_str() {
                "on" | "true" | "yes" | "1" => true,
                "off" | "false" | "no" | "0" => false,
                _ => {
                    show_command_result(
                        state,
                        "Error".to_string(),
                        "Invalid argument. Use 'on' or 'off'".to_string(),
                    );
                    return Ok(());
                }
            };

            // Create EnableTools command
            let cmd = AgentCommand::EnableTools(enable);

            // Send to agent
            let manager = state.agent_manager.lock().unwrap();
            manager.send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
        }

        "system" => {
            if args.is_empty() {
                show_command_result(
                    state,
                    "Error".to_string(),
                    "System prompt is required".to_string(),
                );
                return Ok(());
            }

            // Create SetSystemPrompt command
            let cmd = AgentCommand::SetSystemPrompt(args.to_string());

            // Send to agent
            let manager = state.agent_manager.lock().unwrap();
            manager.send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
        }

        "reset" => {
            // Create ResetConversation command
            let cmd = AgentCommand::ResetConversation;

            // Send to agent
            let manager = state.agent_manager.lock().unwrap();
            manager.send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
        }

        "thinking" => {
            // Parse the thinking budget argument
            if args.is_empty() {
                show_command_result(
                    state,
                    "Error".to_string(),
                    "Thinking budget (number of tokens) is required".to_string(),
                );
                return Ok(());
            }

            let budget = match args.parse::<usize>() {
                Ok(value) => value,
                Err(_) => {
                    show_command_result(
                        state,
                        "Error".to_string(),
                        "Invalid number format".to_string(),
                    );
                    return Ok(());
                }
            };

            // Create SetThinkingBudget command
            // Get the agent manager
            let manager = state.agent_manager.lock().unwrap();

            // Send the command to the agent
            manager.send_message(
                state.selected_agent_id,
                AgentMessage::Command(AgentCommand::SetThinkingBudget(budget)),
            )?;
        }

        "mcp" => {
            show_command_result(
                state,
                "MCP Configuration".to_string(), 
                "MCP servers must be configured in .termineer/config.json file. Direct connection via command is no longer supported.".to_string()
            );
        }

        // Unknown command
        _ => {
            // Log error message to buffer
            state
                .agent_buffer
                .stdout(&format!(
                    "Unknown command: '{}'. Type /help for available commands.",
                    input
                ))
                .unwrap();
        }
    }

    Ok(())
}

/// Handle pound command for agent switching
pub async fn handle_pound_command(
    state: &mut TuiState,
    cmd: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Create popup for command result
    let command_title = format!("Agent Selection: {}", cmd);
    let mut result = String::new();

    // Parse the agent number from the command
    let agent_str = cmd.trim_start_matches('#').trim();

    // Try to parse as a number first (for ID-based selection)
    if let Ok(agent_id) = agent_str.parse::<u64>() {
        let agent_id = AgentId(agent_id);

        // Check if this agent exists
        let agent_exists = state
            .agent_manager
            .lock()
            .map(|manager| manager.get_agent_handle(agent_id).is_some())
            .unwrap_or(false);

        if agent_exists {
            // Switch to this agent
            state.selected_agent_id = agent_id;

            // Update buffer to show the selected agent's output
            let manager = state.agent_manager.lock().unwrap();
            if let Ok(buffer) = manager.get_agent_buffer(agent_id) {
                state.agent_buffer = buffer;

                // Get agent name from manager
                let agent_name = manager
                    .get_agent_handle(agent_id)
                    .map(|handle| handle.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                result.push_str(&format!("Switched to agent {} [{}]", agent_name, agent_id));
            } else {
                result.push_str(&format!("Failed to get buffer for agent {}", agent_id));
            }
        } else {
            result.push_str(&format!("Agent with ID {} not found", agent_id));
        }
    } else {
        // Try to find agent by name using the manager
        let manager_result = state.agent_manager.lock().ok();

        let agent_info = manager_result.and_then(|manager| {
            manager.get_agent_id_by_name(agent_str).map(|id| {
                let name = manager
                    .get_agent_handle(id)
                    .map(|h| h.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());
                (id, name)
            })
        });

        if let Some((agent_id, name)) = agent_info {
            // Switch to this agent
            state.selected_agent_id = agent_id;

            // Update buffer to show the selected agent's output
            let manager = state.agent_manager.lock().unwrap();
            if let Ok(buffer) = manager.get_agent_buffer(agent_id) {
                state.agent_buffer = buffer;
                result.push_str(&format!("Switched to agent {} [{}]", name, agent_id));
            } else {
                result.push_str(&format!("Failed to get buffer for agent {}", name));
            }
        } else {
            result.push_str(&format!("Agent '{}' not found", agent_str));
        }
    }

    // Show result in popup
    show_command_result(state, command_title, result);

    Ok(())
}

/// Show a command result in the temporary output
pub fn show_command_result(state: &mut TuiState, title: String, content: String) {
    state.temp_output.show(title, content);
}