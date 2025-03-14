//! Input tool for sending mouse and keyboard inputs to windows
//!
//! This tool integrates with screendump and screenshot tools to provide
//! complete UI automation capabilities for agents.

use crate::tools::ToolResult;
use std::env;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(target_os = "macos")]
mod macos;

/// Execute the input tool
pub async fn execute_input(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    let platform = env::consts::OS;
    let command = parse_command(args, body);

    if !silent_mode {
        match &command {
            InputCommand::Click { x, y, window_id, button, double } => {
                let button_str = match button {
                    MouseButton::Left => "left",
                    MouseButton::Right => "right",
                    MouseButton::Middle => "middle",
                };
                let double_str = if *double { "double " } else { "" };
                crate::bprintln!("🖱️ Sending {}{}click at ({},{}) to window '{}' on {} platform...", 
                    double_str, button_str, x, y, window_id, platform);
            },
            InputCommand::Type { text: _, window_id } => {
                crate::bprintln!("⌨️ Typing text to window '{}' on {} platform...", window_id, platform);
            },
            InputCommand::KeyPress { key, modifiers, window_id } => {
                crate::bprintln!("⌨️ Sending key {} with modifiers {} to window '{}' on {} platform...", 
                    key, modifiers.join("+"), window_id, platform);
            },
            InputCommand::Sequence { actions, window_id } => {
                crate::bprintln!("🤖 Executing {} input actions on window '{}' on {} platform...", 
                    actions.len(), window_id, platform);
            },
        }
    }

    // Switch on platform
    match platform {
        "macos" => execute_input_macos(command).await,
        "windows" => execute_input_windows(command).await,
        "linux" => execute_input_linux(command).await,
        _ => ToolResult::error(format!("Input tool not implemented for {} platform", platform)),
    }
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Individual input action
#[derive(Debug, Clone)]
pub enum InputAction {
    /// Click at coordinates
    Click {
        x: i32,
        y: i32,
        button: MouseButton,
        double: bool,
    },
    /// Type text
    Type {
        text: String,
    },
    /// Press a keyboard shortcut
    KeyPress {
        key: String,
        modifiers: Vec<String>,
    },
    /// Wait for some milliseconds
    Wait {
        ms: u64,
    },
}

/// Command types for the input tool
#[derive(Debug, Clone)]
pub enum InputCommand {
    /// Click at coordinates in a window
    Click {
        x: i32,
        y: i32,
        window_id: String,
        button: MouseButton, 
        double: bool,
    },
    /// Type text into a window
    Type {
        text: String,
        window_id: String,
    },
    /// Press a keyboard shortcut
    KeyPress {
        key: String,
        modifiers: Vec<String>,
        window_id: String,
    },
    /// Execute a sequence of actions
    Sequence {
        actions: Vec<InputAction>,
        window_id: String,
    },
}

/// Parse command arguments and body into a structured command
fn parse_command(args: &str, body: &str) -> InputCommand {
    let args = args.trim();
    let parts: Vec<&str> = args.split_whitespace().collect();
    
    if parts.is_empty() {
        return InputCommand::Type {
            text: String::new(),
            window_id: String::new(),
        };
    }

    match parts[0].to_lowercase().as_str() {
        "click" => {
            // Format: input click x y [window_id] [options]
            // Options: --right --middle --double
            if parts.len() < 3 {
                return InputCommand::Click {
                    x: 0,
                    y: 0,
                    window_id: String::new(),
                    button: MouseButton::Left,
                    double: false,
                };
            }

            // Parse x,y coordinates
            let x = parts[1].parse::<i32>().unwrap_or(0);
            let y = parts[2].parse::<i32>().unwrap_or(0);

            // Default values
            let mut window_id = String::new();
            let mut button = MouseButton::Left;
            let mut double = false;

            // Parse remaining arguments
            for i in 3..parts.len() {
                match parts[i] {
                    "--right" => button = MouseButton::Right,
                    "--middle" => button = MouseButton::Middle,
                    "--double" => double = true,
                    id => {
                        // Assume it's the window ID if not an option
                        if !id.starts_with("--") {
                            window_id = id.to_string();
                        }
                    }
                }
            }

            InputCommand::Click {
                x,
                y,
                window_id,
                button,
                double,
            }
        },
        "type" => {
            // Format: input type [window_id]
            // The text to type is provided in the body
            let window_id = if parts.len() > 1 { parts[1].to_string() } else { String::new() };
            let text = if body.is_empty() {
                // If no body, check if there's text in the args after 'type'
                if parts.len() > 2 {
                    parts[2..].join(" ")
                } else {
                    String::new()
                }
            } else {
                body.to_string()
            };

            InputCommand::Type {
                text,
                window_id,
            }
        },
        "key" => {
            // Format: input key [modifiers+]key [window_id]
            // Example: input key cmd+shift+a Terminal
            if parts.len() < 2 {
                return InputCommand::KeyPress {
                    key: String::new(),
                    modifiers: vec![],
                    window_id: String::new(),
                };
            }

            let key_combo = parts[1].to_string();
            let window_id = if parts.len() > 2 { parts[2].to_string() } else { String::new() };

            // Parse key combination
            let key_parts: Vec<&str> = key_combo.split('+').collect();
            if key_parts.is_empty() {
                return InputCommand::KeyPress {
                    key: String::new(),
                    modifiers: vec![],
                    window_id,
                };
            }

            // Last part is the key, everything before is modifiers
            let key = key_parts.last().unwrap().to_string();
            let modifiers = key_parts[..key_parts.len()-1]
                .iter()
                .map(|m| m.to_string())
                .collect();

            InputCommand::KeyPress {
                key,
                modifiers,
                window_id,
            }
        },
        "sequence" => {
            // A sequence of actions to perform
            // Format: input sequence [window_id]
            // Actions are provided as JSON in the body
            let window_id = if parts.len() > 1 { parts[1].to_string() } else { String::new() };
            
            // Parse the actions from the body
            let actions = parse_action_sequence(body);
            
            InputCommand::Sequence {
                actions,
                window_id,
            }
        },
        _ => {
            // Default to type command
            InputCommand::Type {
                text: args.to_string(),
                window_id: String::new(),
            }
        }
    }
}

/// Parse a sequence of actions from JSON or text format
fn parse_action_sequence(body: &str) -> Vec<InputAction> {
    let mut actions = Vec::new();
    
    // If body starts with '[', try to parse as JSON
    if body.trim().starts_with('[') {
        if let Ok(json_actions) = serde_json::from_str::<Vec<serde_json::Value>>(body) {
            for action in json_actions {
                if let Some(action_type) = action.get("type").and_then(|t| t.as_str()) {
                    match action_type {
                        "click" => {
                            let x = action.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let y = action.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                            let button_str = action.get("button").and_then(|v| v.as_str()).unwrap_or("left");
                            let double = action.get("double").and_then(|v| v.as_bool()).unwrap_or(false);
                            
                            let button = match button_str {
                                "right" => MouseButton::Right,
                                "middle" => MouseButton::Middle,
                                _ => MouseButton::Left,
                            };
                            
                            actions.push(InputAction::Click { x, y, button, double });
                        },
                        "type" => {
                            let text = action.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            actions.push(InputAction::Type { text });
                        },
                        "key" => {
                            let key = action.get("key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let modifiers = action.get("modifiers").and_then(|v| v.as_array())
                                .map(|arr| arr.iter()
                                    .filter_map(|m| m.as_str().map(|s| s.to_string()))
                                    .collect())
                                .unwrap_or_else(Vec::new);
                            
                            actions.push(InputAction::KeyPress { key, modifiers });
                        },
                        "wait" => {
                            let ms = action.get("ms").and_then(|v| v.as_u64()).unwrap_or(100);
                            actions.push(InputAction::Wait { ms });
                        },
                        _ => {}
                    }
                }
            }
        }
    } else {
        // Parse line by line as simple commands
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }
            
            match parts[0].to_lowercase().as_str() {
                "click" => {
                    if parts.len() < 3 {
                        continue;
                    }
                    
                    let x = parts[1].parse::<i32>().unwrap_or(0);
                    let y = parts[2].parse::<i32>().unwrap_or(0);
                    let mut button = MouseButton::Left;
                    let mut double = false;
                    
                    for i in 3..parts.len() {
                        match parts[i] {
                            "--right" => button = MouseButton::Right,
                            "--middle" => button = MouseButton::Middle,
                            "--double" => double = true,
                            _ => {}
                        }
                    }
                    
                    actions.push(InputAction::Click { x, y, button, double });
                },
                "type" => {
                    let text = if parts.len() > 1 {
                        parts[1..].join(" ")
                    } else {
                        String::new()
                    };
                    
                    actions.push(InputAction::Type { text });
                },
                "key" => {
                    if parts.len() < 2 {
                        continue;
                    }
                    
                    let key_combo = parts[1].to_string();
                    let key_parts: Vec<&str> = key_combo.split('+').collect();
                    
                    if key_parts.is_empty() {
                        continue;
                    }
                    
                    let key = key_parts.last().unwrap().to_string();
                    let modifiers = key_parts[..key_parts.len()-1]
                        .iter()
                        .map(|m| m.to_string())
                        .collect();
                    
                    actions.push(InputAction::KeyPress { key, modifiers });
                },
                "wait" => {
                    let ms = if parts.len() > 1 {
                        parts[1].parse::<u64>().unwrap_or(100)
                    } else {
                        100
                    };
                    
                    actions.push(InputAction::Wait { ms });
                },
                _ => {}
            }
        }
    }
    
    actions
}

/// Execute a sequence of actions with the given window ID
async fn execute_action_sequence(actions: &[InputAction], window_id: &str, platform: &str) -> ToolResult {
    let mut results = Vec::new();
    
    for (index, action) in actions.iter().enumerate() {
        // Execute the action
        let result = match platform {
            "macos" => {
                match action {
                    InputAction::Click { x, y, button, double } => {
                        macos::send_mouse_click(*x, *y, window_id, *button, *double).await
                    },
                    InputAction::Type { text } => {
                        macos::send_keyboard_text(text, window_id).await
                    },
                    InputAction::KeyPress { key, modifiers } => {
                        macos::send_keyboard_shortcut(key, modifiers, window_id).await
                    },
                    InputAction::Wait { ms } => {
                        sleep(Duration::from_millis(*ms)).await;
                        Ok(format!("Waited for {}ms", ms))
                    }
                }
            },
            _ => Err(format!("Input actions not implemented for {} platform", platform))
        };
        
        // Process the result
        match result {
            Ok(msg) => {
                results.push(format!("Action {}: {}", index + 1, msg));
            },
            Err(err) => {
                results.push(format!("⚠️ Action {} failed: {}", index + 1, err));
                // Continue with remaining actions even if one fails
            }
        }
        
        // Small delay between actions for stability
        sleep(Duration::from_millis(50)).await;
    }
    
    ToolResult::success(results.join("\n"))
}

/// Execute the input command on macOS
async fn execute_input_macos(command: InputCommand) -> ToolResult {
    match command {
        InputCommand::Click { x, y, window_id, button, double } => {
            match macos::send_mouse_click(x, y, &window_id, button, double).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        },
        InputCommand::Type { text, window_id } => {
            match macos::send_keyboard_text(&text, &window_id).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        },
        InputCommand::KeyPress { key, modifiers, window_id } => {
            match macos::send_keyboard_shortcut(&key, &modifiers, &window_id).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        },
        InputCommand::Sequence { actions, window_id } => {
            execute_action_sequence(&actions, &window_id, "macos").await
        },
    }
}

/// Execute the input command on Windows (not implemented yet)
async fn execute_input_windows(_command: InputCommand) -> ToolResult {
    ToolResult::error("Input tool not implemented for Windows platform yet")
}

/// Execute the input command on Linux (not implemented yet)
async fn execute_input_linux(_command: InputCommand) -> ToolResult {
    ToolResult::error("Input tool not implemented for Linux platform yet")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command() {
        // Test click command
        match parse_command("click 100 200 Terminal", "") {
            InputCommand::Click { x, y, window_id, button, double } => {
                assert_eq!(x, 100);
                assert_eq!(y, 200);
                assert_eq!(window_id, "Terminal");
                assert_eq!(button, MouseButton::Left);
                assert_eq!(double, false);
            },
            _ => panic!("Expected Click command"),
        }

        // Test click with options
        match parse_command("click 100 200 Terminal --right --double", "") {
            InputCommand::Click { x, y, window_id, button, double } => {
                assert_eq!(x, 100);
                assert_eq!(y, 200);
                assert_eq!(window_id, "Terminal");
                assert_eq!(button, MouseButton::Right);
                assert_eq!(double, true);
            },
            _ => panic!("Expected Click command with options"),
        }

        // Test type command
        match parse_command("type Terminal", "Hello, world!") {
            InputCommand::Type { text, window_id } => {
                assert_eq!(text, "Hello, world!");
                assert_eq!(window_id, "Terminal");
            },
            _ => panic!("Expected Type command"),
        }

        // Test key command
        match parse_command("key cmd+shift+a Terminal", "") {
            InputCommand::KeyPress { key, modifiers, window_id } => {
                assert_eq!(key, "a");
                assert_eq!(modifiers, vec!["cmd", "shift"]);
                assert_eq!(window_id, "Terminal");
            },
            _ => panic!("Expected KeyPress command"),
        }
    }

    #[test]
    fn test_parse_action_sequence() {
        // Test JSON format
        let json = r#"[
            {"type": "click", "x": 100, "y": 200, "button": "left", "double": false},
            {"type": "type", "text": "Hello, world!"},
            {"type": "key", "key": "a", "modifiers": ["cmd", "shift"]},
            {"type": "wait", "ms": 500}
        ]"#;
        
        let actions = parse_action_sequence(json);
        assert_eq!(actions.len(), 4);
        
        // Test text format
        let text = r#"
            click 100 200 --right
            type Hello, world!
            key cmd+shift+a
            wait 500
        "#;
        
        let actions = parse_action_sequence(text);
        assert_eq!(actions.len(), 4);
    }
}