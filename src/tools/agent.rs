//! Agent tool for creating agents and enabling communication between agents
//!
//! This module provides the agent tool with subcommands:
//! - create: Create a new agent
//! - send: Send a message to another agent

use crate::agent::{AgentId, AgentMessage};
use crate::config::Config;
use crate::constants::{FORMAT_BOLD, FORMAT_RESET};
use crate::tools::ToolResult;

/// Execute the agent tool with the given arguments and body
pub async fn execute_agent_tool(
    args: &str,
    body: &str,
    silent_mode: bool,
    source_agent_id: Option<AgentId>,
) -> ToolResult {
    // Check if user has Plus or Pro subscription for multi-agent capabilities
    let app_mode = crate::config::get_app_mode();
    let has_required_subscription = matches!(
        app_mode,
        crate::config::AppMode::Plus | crate::config::AppMode::Pro
    );

    // If user does not have required subscription, return error with upgrade message
    if !has_required_subscription {
        let error_msg = "Multi-agent capabilities require a Plus or Pro subscription.".to_string();
        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Parse the subcommand and arguments
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let subcommand = parts.get(0).map(|s| s.trim()).unwrap_or("");
    let subcommand_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match subcommand {
        "create" => execute_create_subcommand(subcommand_args, body, silent_mode).await,
        "send" => {
            execute_send_subcommand(subcommand_args, body, silent_mode, source_agent_id).await
        }
        _ => {
            let error_msg = format!(
                "Unknown agent subcommand: '{}'. Available subcommands: create, send",
                subcommand
            );
            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }
            ToolResult::error(error_msg)
        }
    }
}

/// Execute the 'create' subcommand to spawn a new agent
async fn execute_create_subcommand(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse the agent name and check for parameters using key=value syntax
    let args_string = args.trim().to_string();
    let mut kind_name = None;

    // Split the args by spaces to check for parameters with key=value syntax
    let parts: Vec<&str> = args_string.split_whitespace().collect();
    let mut agent_name_parts = Vec::new();

    for part in parts {
        if part.starts_with("kind=") {
            // Extract kind parameter
            if let Some(value) = part.strip_prefix("kind=") {
                kind_name = Some(value.to_string());
            }
        } else {
            // This is part of the agent name
            agent_name_parts.push(part);
        }
    }

    // Reconstruct the agent name from non-parameter parts
    let agent_name = agent_name_parts.join(" ");

    // Ensure we have a valid agent name
    if agent_name.is_empty() {
        let error_msg = "Error: Agent creation requires a name".to_string();
        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Check if body is empty
    let agent_instructions = body.trim();
    if agent_instructions.is_empty() {
        let error_msg = "Error: Agent creation requires instructions in the body".to_string();
        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Create a configuration for the new agent
    let mut config = Config::new();

    // Set the kind parameter if provided
    config.kind = kind_name.clone();

    // Log the agent creation
    if !silent_mode {
        if let Some(kind) = &kind_name {
            bprintln !(tool: "agent",
                "Creating agent '{}' with kind '{}'",
                agent_name,
                kind
            );
        } else {
            bprintln !(tool: "agent",
                "Creating agent '{}'",
                agent_name
            );
        }
    }

    // Create the new agent
    let agent_id = match crate::agent::create_agent(agent_name.clone(), config) {
        Ok(id) => id,
        Err(e) => {
            let error_msg = format!("Failed to create agent: {}", e);
            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    };

    // Send the initial instructions to the new agent
    match crate::agent::send_message(
        agent_id,
        AgentMessage::UserInput(agent_instructions.to_string()),
    ) {
        Ok(_) => {
            if !silent_mode {
                bprintln !(tool: "agent",
                    "{}✅ Agent Created:{} {} [ID: {}]",
                    FORMAT_BOLD,
                    FORMAT_RESET,
                    agent_name,
                    agent_id
                );
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to send instructions to new agent: {}", e);
            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    }

    // Return success with agent ID
    ToolResult::default(
        true,
        format!(
            "Agent '{}' created with ID: {}\nInitial instructions sent to the agent.",
            agent_name, agent_id
        ),
    )
}

/// Execute the 'send' subcommand to send a message to another agent
async fn execute_send_subcommand(
    args: &str,
    body: &str,
    silent_mode: bool,
    source_agent_id: Option<AgentId>,
) -> ToolResult {
    // Parse the target agent identifier (name or ID)
    let target_agent = args.trim();
    if target_agent.is_empty() {
        let error_msg = "Error: send subcommand requires a target agent (name or ID)".to_string();
        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Check if body is empty
    let message_content = body.trim();
    if message_content.is_empty() {
        let error_msg = "Error: Message content is required in the body".to_string();
        if !silent_mode {
            bprintln !(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Determine if the target is an ID or name
    let target_id;

    // First try to parse as an ID
    if let Ok(id_num) = target_agent.parse::<u64>() {
        let agent_id = AgentId(id_num);

        // Check if this agent exists
        let agents = crate::agent::get_agents();
        let agent_exists = agents.iter().any(|(id, _)| *id == agent_id);

        if !agent_exists {
            let error_msg = format!("Error: Agent with ID {} not found", id_num);
            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }

        target_id = agent_id;
    } else {
        // Try to find by name
        match crate::agent::get_agent_id_by_name(target_agent) {
            Some(id) => target_id = id,
            None => {
                let error_msg = format!("Error: Agent with name '{}' not found", target_agent);
                if !silent_mode {
                    bprintln !(error:"{}", error_msg);
                }
                return ToolResult::error(error_msg);
            }
        }
    }

    // Get the source agent name and ID for the message formatting
    let agents = crate::agent::get_agents();
    let (source_agent_name, source_id_str) = match source_agent_id {
        Some(id) => {
            // Try to get the actual agent name if source_agent_id is provided
            let name = agents
                .iter()
                .find(|(agent_id, _)| *agent_id == id)
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| format!("agent-{}", id));
            (name, id.to_string())
        }
        None => ("unknown_agent".to_string(), "unknown".to_string()),
    };

    // Format the message with XML tags to indicate it's from another agent
    let formatted_message = format!(
        "<agent_message source=\"{}\" source_id=\"{}\">\n{}\n</agent_message>",
        source_agent_name, source_id_str, message_content
    );

    // Send the message to the target agent
    match crate::agent::send_message(target_id, AgentMessage::UserInput(formatted_message)) {
        Ok(_) => {
            if !silent_mode {
                bprintln !(tool: "agent",
                    "{}✅ Message Sent:{} to agent {}",
                    FORMAT_BOLD,
                    FORMAT_RESET,
                    target_agent
                );
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to send message to agent: {}", e);
            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    }

    // Return success
    ToolResult::default(
        true,
        format!("Message sent to agent {} [ID: {}]", target_agent, target_id),
    )
}
