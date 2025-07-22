//! Command processing for the Terminal UI

use crate::agent::types::AgentCommand;
use crate::agent::{AgentId, AgentMessage};
use crate::tui::state::TuiState;

/// Process slash commands
pub async fn process_command(state: &mut TuiState, input: &str) -> anyhow::Result<()> {
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
            crate::agent::interrupt_agent_with_reason(
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
            crate::agent::send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
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
            crate::agent::send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
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
            crate::agent::send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
        }

        "reset" => {
            // Create ResetConversation command
            let cmd = AgentCommand::ResetConversation;

            // Send to agent
            crate::agent::send_message(state.selected_agent_id, AgentMessage::Command(cmd))?;
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
            // Send the command to the agent
            crate::agent::send_message(
                state.selected_agent_id,
                AgentMessage::Command(AgentCommand::SetThinkingBudget(budget)),
            )?;
        }

        // Unknown command
        _ => {
            // Log error message to buffer
            state
                .agent_buffer
                .stdout(&format!(
                    "Unknown command: '{input}'. Type /help for available commands."
                ))
                .unwrap();
        }
    }

    Ok(())
}

/// Handle pound command for agent switching
pub async fn handle_pound_command(state: &mut TuiState, cmd: &str) -> anyhow::Result<()> {
    // Create popup for command result
    let command_title = format!("Agent Selection: {cmd}");
    let mut result = String::new();

    // Parse the agent number from the command
    let agent_str = cmd.trim_start_matches('#').trim();

    // Try to parse as a number first (for ID-based selection)
    if let Ok(agent_id) = agent_str.parse::<u64>() {
        let agent_id = AgentId(agent_id);

        // Get the list of all agents to check if this agent exists
        let agents = crate::agent::get_agents();
        let agent_exists = agents.iter().any(|(id, _)| *id == agent_id);

        if agent_exists {
            // Switch to this agent
            state.selected_agent_id = agent_id;

            // Update buffer to show the selected agent's output
            if let Ok(buffer) = crate::agent::get_agent_buffer(agent_id) {
                state.agent_buffer = buffer;

                // Get agent name from the agents list
                let agent_name = agents
                    .iter()
                    .find(|(id, _)| *id == agent_id)
                    .map(|(_, name)| name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                result.push_str(&format!("Switched to agent {agent_name} [{agent_id}]"));
            } else {
                result.push_str(&format!("Failed to get buffer for agent {agent_id}"));
            }
        } else {
            result.push_str(&format!("Agent with ID {agent_id} not found"));
        }
    } else {
        // Try to find agent by name
        if let Some(agent_id) = crate::agent::get_agent_id_by_name(agent_str) {
            // Switch to this agent
            state.selected_agent_id = agent_id;

            // Update buffer to show the selected agent's output
            if let Ok(buffer) = crate::agent::get_agent_buffer(agent_id) {
                state.agent_buffer = buffer;
                result.push_str(&format!("Switched to agent {agent_str} [{agent_id}]"));
            } else {
                result.push_str(&format!("Failed to get buffer for agent {agent_str}"));
            }
        } else {
            result.push_str(&format!("Agent '{agent_str}' not found"));
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
