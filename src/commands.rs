//! Command handling for the CLI interface
//!
//! This module provides a clean way to handle commands entered in the CLI.

use crate::agent::Agent;
use crate::constants;
use crate::session;
use std::error::Error;
use std::io::{self, Write};

/// Handle a command from the user
pub fn handle_command(
    client: &mut Agent, 
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
            print!("Conversation history cleared.\r\n");
            io::stdout().flush()?;
        },
        "system" => handle_system_command(client, args)?,
        "model" => handle_model_command(client, args)?,
        "tools" => handle_tools_command(client, args)?,
        "thinking" => handle_thinking_command(client, args)?,
        "session" => handle_session_command(client, args)?,
        _ => {
            print!("Unknown command. Type /help for available commands.\r\n");
            io::stdout().flush()?;
        },
    }
    
    Ok(())
}

/// Display help information
fn display_help() {
    let help_text = constants::format_template(constants::HELP_TEMPLATE);
    print!("{}\r\n", help_text);
    io::stdout().flush().unwrap();
}

/// Handle the /system command
fn handle_system_command(client: &mut Agent, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        print!("Usage: /system YOUR SYSTEM PROMPT TEXT\r\n");
        io::stdout().flush()?;
        return Ok(());
    }
    
    let system_prompt = args.join(" ");
    client.set_system_prompt(system_prompt);
    print!("System prompt set.\r\n");
    io::stdout().flush()?;
    
    Ok(())
}

/// Handle the /model command
fn handle_model_command(client: &mut Agent, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        print!("Usage: /model MODEL_NAME\r\n");
        print!("Examples: claude-3-opus-20240229, claude-3-sonnet-20240229, claude-3-haiku-20240307\r\n");
        io::stdout().flush()?;
        return Ok(());
    }
    
    let model_name = args[0].to_string();
    client.set_model(model_name);
    print!("Model changed.\r\n");
    io::stdout().flush()?;
    
    Ok(())
}

/// Handle the /tools command
fn handle_tools_command(client: &mut Agent, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        print!("Usage: /tools on|off\r\n");
        io::stdout().flush()?;
        return Ok(());
    }
    
    match args[0].to_lowercase().as_str() {
        "on" | "enable" | "true" => {
            client.enable_tools(true);
            print!("Tools enabled. The assistant will use tools automatically based on your request.\r\n");
            io::stdout().flush()?;
        },
        "off" | "disable" | "false" => {
            client.enable_tools(false);
            print!("Tools disabled.\r\n");
            io::stdout().flush()?;
        },
        _ => {
            print!("Usage: /tools on|off\r\n");
            io::stdout().flush()?;
        },
    }
    
    Ok(())
}

/// Handle the /thinking command
fn handle_thinking_command(client: &mut Agent, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        print!("Usage: /thinking NUMBER\r\n");
        io::stdout().flush()?;
        return Ok(());
    }
    
    if let Ok(budget) = args[0].parse::<usize>() {
        client.set_thinking_budget(budget);
        print!("Thinking budget set to {} tokens.\r\n", budget);
    } else {
        print!("Invalid number format. Usage: /thinking NUMBER\r\n");
    }
    io::stdout().flush()?;
    
    Ok(())
}

/// Handle the session command and its subcommands
fn handle_session_command(client: &mut Agent, args: &[&str]) -> Result<(), Box<dyn Error>> {
    if args.is_empty() {
        print!("Usage: /session <command> [args]\r\n");
        print!("Available commands:\r\n");
        print!("  list    - List sessions in current directory\r\n");
        print!("  all     - List sessions from all directories\r\n");
        print!("  save    - Save current session (e.g., /session save my_session)\r\n");
        print!("  load    - Load a session (e.g., /session load my_session)\r\n");
        print!("  delete  - Delete a session (e.g., /session delete my_session)\r\n");
        print!("  resume  - Resume the last session\r\n");
        io::stdout().flush()?;
        return Ok(());
    }
    
    let subcommand = args[0].to_lowercase();
    
    match subcommand.as_str() {
        "list" => {
            match session::list_sessions(client) {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        print!("No saved sessions found.\r\n");
                    } else {
                        print!("\r\nAvailable sessions:\r\n");
                        for (i, session) in sessions.iter().enumerate() {
                            // Format timestamp as date/time
                            let dt = chrono::DateTime::from_timestamp(session.timestamp as i64, 0)
                                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                .unwrap_or_else(|| "Unknown date".to_string());

                            print!("{}. {} (ID: {}, created: {}, messages: {})\r\n",
                                     i + 1,
                                     session.name,
                                     session.id,
                                     dt,
                                     session.metadata.message_count);
                        }

                        print!("\r\nTip: You can load a session with: /session load <name or ID>\r\n");
                    }
                    io::stdout().flush()?;
                }
                Err(e) => {
                    print!("Error listing sessions: {}\r\n", e);
                    io::stdout().flush()?;
                },
            }
        }
        
        "all" => {
            match session::list_all_sessions(client) {
                Ok(all_sessions) => {
                    if all_sessions.is_empty() {
                        print!("No saved sessions found in any directory.\r\n");
                    } else {
                        print!("\r\nSessions from all directories:\r\n");

                        for (dir_name, sessions) in all_sessions {
                            print!("\r\nDirectory: {}\r\n", dir_name);

                            for (i, session) in sessions.iter().enumerate() {
                                // Format timestamp as date/time
                                let dt = chrono::DateTime::from_timestamp(session.timestamp as i64, 0)
                                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                                    .unwrap_or_else(|| "Unknown date".to_string());

                                print!("  {}. {} (ID: {}, created: {})\r\n",
                                         i + 1,
                                         session.name,
                                         session.id,
                                         dt);
                            }
                        }
                    }
                    io::stdout().flush()?;
                }
                Err(e) => {
                    print!("Error listing all sessions: {}\r\n", e);
                    io::stdout().flush()?;
                },
            }
        }
        
        "save" => {
            if args.len() > 1 {
                let name = args[1].to_string();
                match session::save_session(client, &name) {
                    Ok(session_id) => {
                        print!("Session '{}' saved with ID: {}\r\n", name, session_id);
                        print!("You can load it later with: /session load {}\r\n", session_id);
                    }
                    Err(e) => print!("Error saving session: {}\r\n", e),
                }
            } else {
                print!("Usage: /session save <SESSION_NAME>\r\n");
            }
            io::stdout().flush()?;
        }
        
        "load" => {
            if args.len() > 1 {
                let session_id_or_name = args[1].to_string();
                match session::load_session(client, &session_id_or_name) {
                    Ok(_) => print!("Session loaded successfully\r\n"),
                    Err(e) => print!("Error loading session: {}\r\n", e),
                }
            } else {
                print!("Usage: /session load <SESSION_ID_OR_NAME>\r\n");
                print!("Tip: You can also load by name\r\n");
                print!("Use '/session list' to see available sessions\r\n");
            }
            io::stdout().flush()?;
        }
        
        "delete" => {
            if args.len() > 1 {
                let session_id_or_name = args[1].to_string();
                match session::delete_session(client, &session_id_or_name) {
                    Ok(_) => print!("Session deleted successfully\r\n"),
                    Err(e) => print!("Error deleting session: {}\r\n", e),
                }
            } else {
                print!("Usage: /session delete <SESSION_ID_OR_NAME>\r\n");
                print!("Tip: You can also delete by name\r\n");
                print!("Use '/session list' to see available sessions\r\n");
            }
            io::stdout().flush()?;
        }
        
        "resume" => {
            match session::load_last_session(client) {
                Ok(_) => print!("Last session resumed successfully\r\n"),
                Err(e) => print!("Error resuming last session: {}\r\n", e),
            }
            io::stdout().flush()?;
        }
        
        _ => {
            print!("Unknown session command: {}\r\n", subcommand);
            print!("Available commands: list, all, save, load, delete, resume\r\n");
            io::stdout().flush()?;
        }
    }
    
    Ok(())
}