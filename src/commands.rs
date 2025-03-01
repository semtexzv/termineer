//! Command handling for the CLI interface
//!
//! This module provides a clean way to handle commands entered in the CLI.

use crate::ClaudeClient;
use crate::constants;
use crate::session;
use std::error::Error;

/// Handle a command from the user
pub fn handle_command(
    client: &mut ClaudeClient, 
    input: &str
) -> Result<(), Box<dyn Error>> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() { 
        return Ok(()); 
    }
    
    let command = parts[0].trim_start_matches('/').to_lowercase();
    let args = &parts[1..];
    
    match command.as_str() {
        "exit" => return Ok(()),
        "help" => display_help(),
        "clear" => {
            client.clear_conversation();
            println!("Conversation history cleared.");
        },
        "system" => handle_system_command(client, args)?,
        "model" => handle_model_command(client, args)?,
        "tools" => handle_tools_command(client, args)?,
        "thinking" => handle_thinking_command(client, args)?,
        "session" => handle_session_command(client, args)?,
        _ => println!("Unknown command. Type /help for available commands."),
    }
    
    Ok(())
}

/// Display help information
fn display_help() {
    let help_text = constants::format_template(constants::HELP_TEMPLATE);
    println!("{}", help_text);
}

/// Handle the /system command
fn handle_system_command(client: &mut ClaudeClient, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("Usage: /system YOUR SYSTEM PROMPT TEXT");
        return Ok(());
    }
    
    let system_prompt = args.join(" ");
    client.set_system_prompt(system_prompt);
    println!("System prompt set.");
    
    Ok(())
}

/// Handle the /model command
fn handle_model_command(client: &mut ClaudeClient, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("Usage: /model MODEL_NAME");
        println!("Examples: claude-3-opus-20240229, claude-3-sonnet-20240229, claude-3-haiku-20240307");
        return Ok(());
    }
    
    let model_name = args[0].to_string();
    client.set_model(model_name);
    println!("Model changed.");
    
    Ok(())
}

/// Handle the /tools command
fn handle_tools_command(client: &mut ClaudeClient, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("Usage: /tools on|off");
        return Ok(());
    }
    
    match args[0].to_lowercase().as_str() {
        "on" | "enable" | "true" => {
            client.enable_tools(true);
            println!("Tools enabled. Claude will use tools automatically based on your request.");
        },
        "off" | "disable" | "false" => {
            client.enable_tools(false);
            println!("Tools disabled.");
        },
        _ => println!("Usage: /tools on|off"),
    }
    
    Ok(())
}

/// Handle the /thinking command
fn handle_thinking_command(client: &mut ClaudeClient, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("Usage: /thinking NUMBER");
        return Ok(());
    }
    
    if let Ok(budget) = args[0].parse::<usize>() {
        client.set_thinking_budget(budget);
        println!("Thinking budget set to {} tokens.", budget);
    } else {
        println!("Invalid number format. Usage: /thinking NUMBER");
    }
    
    Ok(())
}

/// Handle the session command and its subcommands
fn handle_session_command(client: &mut ClaudeClient, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        println!("Usage: /session <command> [args]");
        println!("Available commands:");
        println!("  list    - List sessions in current directory");
        println!("  all     - List sessions from all directories");
        println!("  save    - Save current session (e.g., /session save my_session)");
        println!("  load    - Load a session (e.g., /session load my_session)");
        println!("  delete  - Delete a session (e.g., /session delete my_session)");
        println!("  resume  - Resume the last session");
        return Ok(());
    }
    
    let subcommand = args[0].to_lowercase();
    
    match subcommand.as_str() {
        "list" => {
            match session::list_sessions(client) {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No saved sessions found.");
                    } else {
                        println!("\nAvailable sessions:");
                        for (i, session) in sessions.iter().enumerate() {
                            // Format timestamp as date/time
                            let dt = chrono::DateTime::from_timestamp(session.timestamp as i64, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| "Unknown date".to_string());

                            println!("{}. {} (ID: {}, created: {}, messages: {})",
                                     i + 1,
                                     session.name,
                                     session.id,
                                     dt,
                                     session.metadata.message_count);
                        }

                        println!("\nTip: You can load a session with: /session load <name or ID>");
                    }
                }
                Err(e) => println!("Error listing sessions: {}", e),
            }
        }
        
        "all" => {
            match session::list_all_sessions(client) {
                Ok(all_sessions) => {
                    if all_sessions.is_empty() {
                        println!("No saved sessions found in any directory.");
                    } else {
                        println!("\nSessions from all directories:");

                        for (dir_name, sessions) in all_sessions {
                            println!("\nDirectory: {}", dir_name);

                            for (i, session) in sessions.iter().enumerate() {
                                // Format timestamp as date/time
                                let dt = chrono::DateTime::from_timestamp(session.timestamp as i64, 0)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                    .unwrap_or_else(|| "Unknown date".to_string());

                                println!("  {}. {} (ID: {}, created: {})",
                                         i + 1,
                                         session.name,
                                         session.id,
                                         dt);
                            }
                        }
                    }
                }
                Err(e) => println!("Error listing all sessions: {}", e),
            }
        }
        
        "save" => {
            if args.len() > 1 {
                let name = args[1].to_string();
                match session::save_session(client, &name) {
                    Ok(session_id) => {
                        println!("Session '{}' saved with ID: {}", name, session_id);
                        println!("You can load it later with: /session load {}", session_id);
                    }
                    Err(e) => println!("Error saving session: {}", e),
                }
            } else {
                println!("Usage: /session save <SESSION_NAME>");
            }
        }
        
        "load" => {
            if args.len() > 1 {
                let session_id_or_name = args[1].to_string();
                match session::load_session(client, &session_id_or_name) {
                    Ok(_) => println!("Session loaded successfully"),
                    Err(e) => println!("Error loading session: {}", e),
                }
            } else {
                println!("Usage: /session load <SESSION_ID_OR_NAME>");
                println!("Tip: You can also load by name");
                println!("Use '/session list' to see available sessions");
            }
        }
        
        "delete" => {
            if args.len() > 1 {
                let session_id_or_name = args[1].to_string();
                match session::delete_session(client, &session_id_or_name) {
                    Ok(_) => println!("Session deleted successfully"),
                    Err(e) => println!("Error deleting session: {}", e),
                }
            } else {
                println!("Usage: /session delete <SESSION_ID_OR_NAME>");
                println!("Tip: You can also delete by name");
                println!("Use '/session list' to see available sessions");
            }
        }
        
        "resume" => {
            match session::load_last_session(client) {
                Ok(_) => println!("Last session resumed successfully"),
                Err(e) => println!("Error resuming last session: {}", e),
            }
        }
        
        _ => {
            println!("Unknown session command: {}", subcommand);
            println!("Available commands: list, all, save, load, delete, resume");
        }
    }
    
    Ok(())
}