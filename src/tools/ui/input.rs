//! Input tool module
//!
//! This tool allows sending mouse and keyboard inputs to applications

use crate::tools::ToolResult;

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButtonType {
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
        button: MouseButtonType,
        double: bool,
    },
    /// Type text
    Type { text: String },
    /// Press a keyboard shortcut
    KeyPress { key: String, modifiers: Vec<String> },
    /// Wait for some milliseconds
    Wait { ms: u64 },
}

/// Command types for the input tool
#[derive(Debug, Clone)]
pub enum InputCommand {
    /// Click at coordinates in a window
    Click {
        x: i32,
        y: i32,
        window_id: String,
        button: MouseButtonType,
        double: bool,
    },
    /// Type text into a window
    Type { text: String, window_id: String },
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
pub fn parse_command(args: &str, body: &str) -> InputCommand {
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
                    button: MouseButtonType::Left,
                    double: false,
                };
            }

            // Parse x,y coordinates
            let x = parts[1].parse::<i32>().unwrap_or(0);
            let y = parts[2].parse::<i32>().unwrap_or(0);

            // Default values
            let mut window_id = String::new();
            let mut button = MouseButtonType::Left;
            let mut double = false;

            // Parse remaining arguments
            for i in 3..parts.len() {
                match parts[i] {
                    "--right" => button = MouseButtonType::Right,
                    "--middle" => button = MouseButtonType::Middle,
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
        }
        "type" => {
            // Format: input type [window_id]
            // The text to type is provided in the body
            let window_id = if parts.len() > 1 {
                parts[1].to_string()
            } else {
                String::new()
            };
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

            InputCommand::Type { text, window_id }
        }
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
            let window_id = if parts.len() > 2 {
                parts[2].to_string()
            } else {
                String::new()
            };

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
            let modifiers = key_parts[..key_parts.len() - 1]
                .iter()
                .map(|m| m.to_string())
                .collect();

            InputCommand::KeyPress {
                key,
                modifiers,
                window_id,
            }
        }
        "sequence" => {
            // A sequence of actions to perform
            // Format: input sequence [window_id]
            // Actions are provided as JSON in the body
            let window_id = if parts.len() > 1 {
                parts[1].to_string()
            } else {
                String::new()
            };

            // Parse the actions from the body
            let actions = parse_action_sequence(body);

            InputCommand::Sequence { actions, window_id }
        }
        _ => {
            // Default to type command
            InputCommand::Type {
                text: args.to_string(),
                window_id: String::new(),
            }
        }
    }
}

/// Parse a sequence of actions from body text
pub fn parse_action_sequence(body: &str) -> Vec<InputAction> {
    let mut actions = Vec::new();

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
                let mut button = MouseButtonType::Left;
                let mut double = false;

                for i in 3..parts.len() {
                    match parts[i] {
                        "--right" => button = MouseButtonType::Right,
                        "--middle" => button = MouseButtonType::Middle,
                        "--double" => double = true,
                        _ => {}
                    }
                }

                actions.push(InputAction::Click {
                    x,
                    y,
                    button,
                    double,
                });
            }
            "type" => {
                let text = if parts.len() > 1 {
                    parts[1..].join(" ")
                } else {
                    String::new()
                };

                actions.push(InputAction::Type { text });
            }
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
                let modifiers = key_parts[..key_parts.len() - 1]
                    .iter()
                    .map(|m| m.to_string())
                    .collect();

                actions.push(InputAction::KeyPress { key, modifiers });
            }
            "wait" => {
                let ms = if parts.len() > 1 {
                    parts[1].parse::<u64>().unwrap_or(100)
                } else {
                    100
                };

                actions.push(InputAction::Wait { ms });
            }
            _ => {}
        }
    }

    actions
}

/// Execute the input tool
pub async fn execute_input(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Get platform
    let platform = std::env::consts::OS;

    // Log tool invocation
    crate::bprintln!(dev: "💻 INPUT: execute_input called with args=\"{}\" body_length={} on platform={}", 
                     args, body.len(), platform);

    // Parse the command
    let command = parse_command(args, body);

    // Route to platform-specific implementation
    match platform {
        #[cfg(target_os = "macos")]
        "macos" => crate::tools::ui::macos::input::execute_macos_input(command, silent_mode).await,

        #[cfg(target_os = "windows")]
        "windows" => {
            crate::tools::ui::windows::input::execute_windows_input(command, silent_mode).await
        }

        #[cfg(target_os = "linux")]
        "linux" => crate::tools::ui::linux::input::execute_linux_input(command, silent_mode).await,

        _ => ToolResult::error(format!(
            "Input tool not implemented for {} platform",
            platform
        )),
    }
}
