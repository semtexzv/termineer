//! Command handling for the CLI interface
//!
//! This module provides a clean way to handle commands entered in the CLI.

use std::error::Error;
use std::sync::{Arc, Mutex};

use crate::agent::{AgentManager, AgentCommand, AgentId, AgentMessage};
use crate::constants;
use crate::session;

/// Handle a command from the user
pub async fn handle_command(
    agent_manager: &Arc<Mutex<AgentManager>>,
    active_agent_id: AgentId,
    input: &str,
) -> Result<(), Box<dyn Error>> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(());
    }

    let command = parts[0].trim_start_matches('/').to_lowercase();
    let args = &parts[1..];

    match command.as_str() {
        "exit" => return Ok(()),
        "help" => display_help().await?,
        "clear" => {
            let manager = agent_manager.lock().unwrap();
            manager.send_message(
                active_agent_id,
                AgentMessage::Command(AgentCommand::ResetConversation),
            )?;
            crate::bprintln!("Conversation history cleared.");
        }
        "system" => handle_system_command(agent_manager, active_agent_id, args).await?,
        "model" => handle_model_command(agent_manager, active_agent_id, args).await?,
        "tools" => handle_tools_command(agent_manager, active_agent_id, args).await?,
        "thinking" => handle_thinking_command(agent_manager, active_agent_id, args).await?,
        "session" => handle_session_command(agent_manager, active_agent_id, args).await?,
        _ => {
            crate::bprintln!("Unknown command. Type /help for available commands.");
        }
    }

    Ok(())
}

/// Display help information
async fn display_help() -> Result<(), Box<dyn Error>> {
    let help_text = constants::format_template(constants::HELP_TEMPLATE);
    crate::bprintln!("{}", help_text);
    Ok(())
}

/// Handle the /system command
async fn handle_system_command(
    agent_manager: &Arc<Mutex<AgentManager>>,
    agent_id: AgentId,
    args: &[&str],
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        crate::bprintln!("Usage: /system YOUR SYSTEM PROMPT TEXT");
        return Ok(());
    }

    let system_prompt = args.join(" ");
    let manager = agent_manager.lock().unwrap();
    manager.send_message(
        agent_id,
        AgentMessage::Command(AgentCommand::SetSystemPrompt(system_prompt)),
    )?;
    crate::bprintln!("System prompt set.");

    Ok(())
}

/// Handle the /model command
async fn handle_model_command(
    agent_manager: &Arc<Mutex<AgentManager>>,
    agent_id: AgentId,
    args: &[&str],
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        crate::bprintln!("Usage: /model MODEL_NAME");
        crate::bprintln!(
            "Examples: claude-3-opus-20240229, claude-3-sonnet-20240229, claude-3-haiku-20240307"
        );
        return Ok(());
    }

    let model_name = args[0].to_string();
    let manager = agent_manager.lock().unwrap();
    manager.send_message(
        agent_id,
        AgentMessage::Command(AgentCommand::SetModel(model_name)),
    )?;
    crate::bprintln!("Model changed.");

    Ok(())
}

/// Handle the /tools command
async fn handle_tools_command(
    agent_manager: &Arc<Mutex<AgentManager>>,
    agent_id: AgentId,
    args: &[&str],
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        crate::bprintln!("Usage: /tools on|off");
        return Ok(());
    }

    match args[0].to_lowercase().as_str() {
        "on" | "enable" | "true" => {
            let manager = agent_manager.lock().unwrap();
            manager.send_message(
                agent_id,
                AgentMessage::Command(AgentCommand::EnableTools(true)),
            )?;
            crate::bprintln!(
                "Tools enabled. The assistant will use tools automatically based on your request."
            );
        }
        "off" | "disable" | "false" => {
            let manager = agent_manager.lock().unwrap();
            manager.send_message(
                agent_id,
                AgentMessage::Command(AgentCommand::EnableTools(false)),
            )?;
            crate::bprintln!("Tools disabled.");
        }
        _ => {
            crate::bprintln!("Usage: /tools on|off");
        }
    }

    Ok(())
}

/// Handle the /thinking command
async fn handle_thinking_command(
    _agent_manager: &Arc<Mutex<AgentManager>>,
    _agent_id: AgentId,
    args: &[&str],
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        crate::bprintln!("Usage: /thinking NUMBER");
        return Ok(());
    }

    if let Ok(_budget) = args[0].parse::<usize>() {
        // TODO: Add thinking budget command to AgentCommand enum
        crate::bprintln!(
            "The thinking budget command is temporarily unavailable in multi-agent mode."
        );
    } else {
        crate::bprintln!("Invalid number format. Usage: /thinking NUMBER");
    }

    Ok(())
}

/// Handle the session command and its subcommands
async fn handle_session_command(
    _agent_manager: &Arc<Mutex<AgentManager>>,
    _agent_id: AgentId,
    args: &[&str],
) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        crate::bprintln!("Usage: /session <command> [args]");
        crate::bprintln!("Available commands:");
        crate::bprintln!("  list    - List sessions in current directory");
        crate::bprintln!("  all     - List sessions from all directories");
        crate::bprintln!("  save    - Save current session (e.g., /session save my_session)");
        crate::bprintln!("  load    - Load a session (e.g., /session load my_session)");
        crate::bprintln!("  delete  - Delete a session (e.g., /session delete my_session)");
        crate::bprintln!("  resume  - Resume the last session");
        return Ok(());
    }

    // TODO: Implement session management for multi-agent mode
    crate::bprintln!("Session management is temporarily unavailable in multi-agent mode.");

    Ok(())
}
