//! Agent tool for creating agents and enabling communication between agents
//!
//! This module provides the agent tool with subcommands:
//! - create: Create a new agent
//! - send: Send a message to another agent
//! - wait: Wait for messages from other agents

use crate::agent::{AgentManager, AgentMessage, AgentId};
use crate::config::Config;
use crate::constants::{FORMAT_BOLD, FORMAT_RESET};
use crate::tools::ToolResult;
use std::sync::{Arc, Mutex};

use crate::GLOBAL_AGENT_MANAGER;

/// Execute the agent tool with the given arguments and body
pub async fn execute_agent_tool(
    args: &str, 
    body: &str, 
    silent_mode: bool,
    source_agent_id: Option<AgentId>
) -> ToolResult {
    // Get access to the agent manager (either provided or global)
    let agent_manager = GLOBAL_AGENT_MANAGER.clone();

    // Parse the subcommand and arguments
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    let subcommand = parts.get(0).map(|s| s.trim()).unwrap_or("");
    let subcommand_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

    match subcommand {
        "create" => {
            execute_create_subcommand(subcommand_args, body, silent_mode, agent_manager).await
        }
        "send" => {
            execute_send_subcommand(subcommand_args, body, silent_mode, agent_manager, source_agent_id).await
        }
        "wait" => {
            execute_wait_subcommand(silent_mode, agent_manager).await
        }
        _ => {
            let error_msg = format!(
                "Unknown agent subcommand: '{}'. Available subcommands: create, send, wait", 
                subcommand
            );
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
            }
            ToolResult::error(error_msg)
        }
    }
}

/// Execute the 'create' subcommand to spawn a new agent
async fn execute_create_subcommand(
    args: &str,
    body: &str,
    silent_mode: bool,
    agent_manager: Arc<Mutex<AgentManager>>,
) -> ToolResult {
    // Parse the agent name (no model parameter allowed)
    let agent_name = args.trim().to_string();

    // Ensure we have a valid agent name
    if agent_name.is_empty() {
        let error_msg = "Error: Agent creation requires a name".to_string();
        if !silent_mode {
            crate::berror_println!("{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Check if body is empty
    let agent_instructions = body.trim();
    if agent_instructions.is_empty() {
        let error_msg = "Error: Agent creation requires instructions in the body".to_string();
        if !silent_mode {
            crate::berror_println!("{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Create a configuration for the new agent
    let config = Config::new();
    
    // Log the agent creation
    if !silent_mode {
        crate::btool_println!(
            "agent",
            "Creating agent '{}'",
            agent_name
        );
    }

    // Create the new agent
    let agent_id = {
        let mut manager = agent_manager.lock().unwrap();
        match manager.create_agent(agent_name.clone(), config) {
            Ok(id) => id,
            Err(e) => {
                let error_msg = format!("Failed to create agent: {}", e);
                if !silent_mode {
                    crate::berror_println!("{}", error_msg);
                }
                return ToolResult::error(error_msg);
            }
        }
    };

    // Send the initial instructions to the new agent
    {
        let manager = agent_manager.lock().unwrap();
        match manager.send_message(
            agent_id,
            AgentMessage::UserInput(agent_instructions.to_string()),
        ) {
            Ok(_) => {
                if !silent_mode {
                    crate::btool_println!(
                        "agent",
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
                    crate::berror_println!("{}", error_msg);
                }
                return ToolResult::error(error_msg);
            }
        }
    }

    // Return success with agent ID
    ToolResult::default(
        true,
        format!(
            "Agent '{}' created with ID: {}\nInitial instructions sent to the agent.",
            agent_name,
            agent_id
        )
    )
}

/// Execute the 'send' subcommand to send a message to another agent
async fn execute_send_subcommand(
    args: &str,
    body: &str,
    silent_mode: bool,
    agent_manager: Arc<Mutex<AgentManager>>,
    source_agent_id: Option<AgentId>,
) -> ToolResult {
    // Parse the target agent identifier (name or ID)
    let target_agent = args.trim();
    if target_agent.is_empty() {
        let error_msg = "Error: send subcommand requires a target agent (name or ID)".to_string();
        if !silent_mode {
            crate::berror_println!("{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Check if body is empty
    let message_content = body.trim();
    if message_content.is_empty() {
        let error_msg = "Error: Message content is required in the body".to_string();
        if !silent_mode {
            crate::berror_println!("{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Determine if the target is an ID or name
    let target_id = {
        let manager = agent_manager.lock().unwrap();

        // First try to parse as an ID
        if let Ok(id_num) = target_agent.parse::<u64>() {
            let agent_id = crate::agent::AgentId(id_num);
            // Verify the agent exists
            if manager.get_agent_handle(agent_id).is_none() {
                let error_msg = format!("Error: Agent with ID {} not found", id_num);
                if !silent_mode {
                    crate::berror_println!("{}", error_msg);
                }
                return ToolResult::error(error_msg);
            }
            agent_id
        } else {
            // Try to find by name
            match manager.get_agent_id_by_name(target_agent) {
                Some(id) => id,
                None => {
                    let error_msg = format!("Error: Agent with name '{}' not found", target_agent);
                    if !silent_mode {
                        crate::berror_println!("{}", error_msg);
                    }
                    return ToolResult::error(error_msg);
                }
            }
        }
    };

    // Get the source agent name and ID for the message formatting
    let (source_agent_name, source_id_str) = match source_agent_id {
        Some(id) => {
            // Try to get the actual agent name if source_agent_id is provided
            let manager = agent_manager.lock().unwrap();
            let name = manager.get_agent_handle(id)
                .map(|h| h.name.clone())
                .unwrap_or_else(|| format!("agent-{}", id));
            (name, id.to_string())
        },
        None => ("unknown_agent".to_string(), "unknown".to_string())
    };

    // Format the message with XML tags to indicate it's from another agent
    let formatted_message = format!(
        "<agent_message source=\"{}\" source_id=\"{}\">\n{}\n</agent_message>",
        source_agent_name,
        source_id_str,
        message_content
    );

    // Send the message to the target agent
    {
        let manager = agent_manager.lock().unwrap();
        match manager.send_message(
            target_id,
            AgentMessage::UserInput(formatted_message),
        ) {
            Ok(_) => {
                if !silent_mode {
                    crate::btool_println!(
                        "agent",
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
                    crate::berror_println!("{}", error_msg);
                }
                return ToolResult::error(error_msg);
            }
        }
    }

    // Return success
    ToolResult::default(
        true,
        format!(
            "Message sent to agent {} [ID: {}]",
            target_agent,
            target_id
        )
    )
}

/// Execute the 'wait' subcommand to wait for messages
async fn execute_wait_subcommand(
    silent_mode: bool,
    _agent_manager: Arc<Mutex<AgentManager>>,
) -> ToolResult {
    if !silent_mode {
        crate::btool_println!(
            "agent",
            "{}⏸️ Waiting:{} Agent will wait for messages",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET
        );
    }
    
    // Use the wait method directly to set the state to Wait
    ToolResult {
        success: true,
        agent_output: "Agent is now waiting: Waiting for messages from other agents\nAny input will resume processing.".to_string(),
        state_change: crate::tools::AgentStateChange::Wait,
    }
}