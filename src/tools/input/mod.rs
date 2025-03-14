//! Input tool for sending mouse and keyboard inputs to windows
//!
//! This tool integrates with screendump and screenshot tools to provide
//! complete UI automation capabilities for agents.

use crate::tools::ToolResult;
use crate::tools::screendump;
use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use std::time::Duration;
use tokio::time::sleep;

/// Execute the input tool
pub async fn execute_input(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    bprintln!(debug: "💻 INPUT: execute_input called with args=\"{}\" body_length={}", args, body.len());
    let command = parse_command(args, body);

    bprintln!(debug: "💻 INPUT: Parsed command: {:?}", command);

    if !silent_mode {
        match &command {
            InputCommand::Click { x, y, window_id, button, double } => {
                let button_str = match button {
                    MouseButtonType::Left => "left",
                    MouseButtonType::Right => "right",
                    MouseButtonType::Middle => "middle",
                };
                let double_str = if *double { "double " } else { "" };
                crate::bprintln!("🖱️ Sending {}{}click at ({},{}) to window '{}' on {} platform...", 
                    double_str, button_str, x, y, window_id, std::env::consts::OS);
            },
            InputCommand::Type { text, window_id } => {
                crate::bprintln!("⌨️ Typing text to window '{}' on {} platform...", window_id, std::env::consts::OS);
                bprintln!(debug: "💻 INPUT: Will type text of length {}: '{}'", text.len(), if text.len() > 50 { format!("{}...", &text[..50]) } else { text.clone() });
            },
            InputCommand::KeyPress { key, modifiers, window_id } => {
                crate::bprintln!("⌨️ Sending key {} with modifiers {} to window '{}' on {} platform...", 
                    key, modifiers.join("+"), window_id, std::env::consts::OS);
            },
            InputCommand::Sequence { actions, window_id } => {
                crate::bprintln!("🤖 Executing {} input actions on window '{}' on {} platform...", 
                    actions.len(), window_id, std::env::consts::OS);
                for (i, action) in actions.iter().enumerate() {
                    bprintln!(debug: "💻 INPUT: Action {}: {:?}", i+1, action);
                }
            },
        }
    }

    match command {
        InputCommand::Click { x, y, window_id, button, double } => {
            match send_mouse_click(x, y, &window_id, button, double).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        },
        InputCommand::Type { text, window_id } => {
            match send_keyboard_text(&text, &window_id).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        },
        InputCommand::KeyPress { key, modifiers, window_id } => {
            match send_keyboard_shortcut(&key, &modifiers, &window_id).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        },
        InputCommand::Sequence { actions, window_id } => {
            execute_action_sequence(&actions, &window_id).await
        },
    }
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseButtonType {
    Left,
    Right,
    Middle,
}

/// Convert MouseButtonType to Enigo's MouseButton
fn to_enigo_button(button: MouseButtonType) -> MouseButton {
    match button {
        MouseButtonType::Left => MouseButton::Left,
        MouseButtonType::Right => MouseButton::Right,
        MouseButtonType::Middle => MouseButton::Middle,
    }
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
        button: MouseButtonType, 
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
                                "right" => MouseButtonType::Right,
                                "middle" => MouseButtonType::Middle,
                                _ => MouseButtonType::Left,
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

/// Check if a key corresponds to a special key in Enigo
fn parse_key(key: &str) -> Option<Key> {
    match key.to_lowercase().as_str() {
        "return" | "enter" => Some(Key::Return),
        "tab" => Some(Key::Tab),
        "space" => Some(Key::Space),
        "backspace" => Some(Key::Backspace),
        "escape" | "esc" => Some(Key::Escape),
        "up" | "uparrow" => Some(Key::UpArrow),
        "down" | "downarrow" => Some(Key::DownArrow),
        "left" | "leftarrow" => Some(Key::LeftArrow),
        "right" | "rightarrow" => Some(Key::RightArrow),
        "home" => Some(Key::Home),
        "end" => Some(Key::End),
        "pageup" => Some(Key::PageUp),
        "pagedown" => Some(Key::PageDown),
        "delete" | "del" => Some(Key::Delete),
        "f1" => Some(Key::F1),
        "f2" => Some(Key::F2),
        "f3" => Some(Key::F3),
        "f4" => Some(Key::F4),
        "f5" => Some(Key::F5),
        "f6" => Some(Key::F6),
        "f7" => Some(Key::F7),
        "f8" => Some(Key::F8),
        "f9" => Some(Key::F9),
        "f10" => Some(Key::F10),
        "f11" => Some(Key::F11),
        "f12" => Some(Key::F12),
        _ => None,
    }
}

/// Parse modifier key
fn parse_modifier(modifier: &str) -> Option<Key> {
    match modifier.to_lowercase().as_str() {
        "command" | "cmd" | "meta" => Some(Key::Meta),
        "shift" => Some(Key::Shift),
        "alt" | "option" => Some(Key::Alt),
        "control" | "ctrl" => Some(Key::Control),
        _ => None,
    }
}

/// Get the window position and size from its ID
async fn get_window_position(window_id: &str) -> Result<(i32, i32), String> {
    bprintln!(debug: "💻 INPUT: Getting position for window '{}'", window_id);
    
    // Get window information from screendump module
    let result = screendump::get_window_rect(window_id);
    
    match &result {
        Ok((app, title, x, y, width, height)) => {
            bprintln!(debug: "💻 INPUT: Window '{}' found: app='{}', title='{}', position=({},{}), size={}x{}", 
                     window_id, app, title, x, y, width, height);
        },
        Err(e) => {
            bprintln!(warn: "💻 INPUT: ⚠️ Failed to get window position: {}", e);
        }
    }
    
    let (_, _, x, y, _, _) = result
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    
    Ok((x, y))
}

/// Activate a window by its ID
async fn activate_window(window_id: &str) -> Result<(), String> {
    bprintln!(debug: "💻 INPUT: Activating window '{}'", window_id);
    
    // This is a bit more complex and platform-specific
    // For now we'll use a simplistic approach, but could be enhanced
    
    // First check if we can get the window rect (validates it exists)
    match screendump::get_window_rect(window_id) {
        Ok(_) => bprintln!(debug: "💻 INPUT: Window '{}' found, will activate", window_id),
        Err(e) => {
            let error = format!("Failed to find window: {}", e);
            bprintln!(error: "💻 INPUT: ⚠️ {}", error);
            return Err(error);
        }
    }
    
    // On macOS we might want to actually focus the window
    // This is a simplified approach that may not work in all cases
    if cfg!(target_os = "macos") {
        let parts: Vec<&str> = window_id.split(':').collect();
        if parts.is_empty() {
            let error = "Invalid window ID format".to_string();
            bprintln!(error: "💻 INPUT: ⚠️ {}", error);
            return Err(error);
        }
        
        let app_name = parts[0];
        bprintln!(debug: "💻 INPUT: Using osascript to activate application '{}'", app_name);
        
        // Try to focus the application using a simple bash command
        // In a more robust implementation, we would use platform APIs
        let script = format!("tell application \"{}\" to activate", app_name);
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| {
                let error = format!("Failed to execute osascript: {}", e);
                bprintln!(error: "💻 INPUT: ⚠️ {}", error);
                error
            })?;
        
        if !output.status.success() {
            let error = String::from_utf8_lossy(&output.stderr);
            let error_msg = format!("Failed to activate window: {}", error);
            bprintln!(error: "💻 INPUT: ⚠️ {}", error_msg);
            return Err(error_msg);
        }
        
        bprintln!(debug: "💻 INPUT: Successfully activated '{}', waiting for activation to complete", app_name);
        // Wait for the activation to complete
        sleep(Duration::from_millis(200)).await;
    } else {
        bprintln!(info: "💻 INPUT: Window activation not implemented for this platform, continuing");
    }
    
    bprintln!(debug: "💻 INPUT: Window activation completed for '{}'", window_id);
    Ok(())
}

/// Send a mouse click at the specified coordinates
async fn send_mouse_click(
    x: i32, 
    y: i32, 
    window_id: &str, 
    button: MouseButtonType,
    double: bool
) -> Result<String, String> {
    bprintln!(debug: "💻 INPUT: Sending mouse click: x={}, y={}, window='{}', button={:?}, double={}", 
              x, y, window_id, button, double);
    
    // First activate the window
    activate_window(window_id).await?;
    
    // Get the window position
    let (win_x, win_y) = get_window_position(window_id).await?;
    
    // Calculate absolute screen coordinates
    let abs_x = win_x + x;
    let abs_y = win_y + y;
    bprintln!(debug: "💻 INPUT: Calculated absolute coordinates: ({}, {})", abs_x, abs_y);
    
    // Use tokio's block_in_place to avoid blocking the runtime
    bprintln!(debug: "💻 INPUT: Executing mouse click with Enigo (blocking)");
    let result = tokio::task::block_in_place(|| -> Result<(), String> {
        // Create a new Enigo instance
        let mut enigo = Enigo::new();
        
        // Move to the position
        bprintln!(dev: "💻 INPUT: Moving mouse to ({}, {})", abs_x, abs_y);
        enigo.mouse_move_to(abs_x, abs_y);
        
        // Perform the click
        let enigo_button = to_enigo_button(button);
        
        // Single or double click
        bprintln!(dev: "💻 INPUT: Performing mouse down with button {:?}", button);
        enigo.mouse_down(enigo_button);
        bprintln!(dev: "💻 INPUT: Performing mouse up");
        enigo.mouse_up(enigo_button);
        
        if double {
            // Small pause between clicks for double-click
            bprintln!(dev: "💻 INPUT: Pausing for double-click");
            std::thread::sleep(Duration::from_millis(10));
            bprintln!(dev: "💻 INPUT: Performing second click (mouse down)");
            enigo.mouse_down(enigo_button);
            bprintln!(dev: "💻 INPUT: Performing second click (mouse up)");
            enigo.mouse_up(enigo_button);
        }
        
        Ok(())
    });
    
    if let Err(e) = &result {
        bprintln!(error: "💻 INPUT: ⚠️ Mouse click failed: {}", e);
    } else {
        bprintln!(debug: "💻 INPUT: Mouse click completed successfully");
    }
    
    result?;
    
    Ok(format!("Clicked at coordinates ({}, {}) in window '{}'", x, y, window_id))
}

/// Type text into a window
async fn send_keyboard_text(text: &str, window_id: &str) -> Result<String, String> {
    bprintln!(debug: "💻 INPUT: Sending keyboard text to window '{}'", window_id);
    if text.len() > 100 {
        bprintln!(debug: "💻 INPUT: Text content (first 100 chars): '{}'...", &text[..100]);
    } else {
        bprintln!(debug: "💻 INPUT: Text content: '{}'", text);
    }
    
    // First activate the window
    activate_window(window_id).await?;
    
    // Small delay to ensure window is active
    bprintln!(debug: "💻 INPUT: Waiting 100ms for window activation to settle");
    sleep(Duration::from_millis(100)).await;
    
    // Use tokio's block_in_place for the Enigo operations
    bprintln!(debug: "💻 INPUT: Executing keyboard input with Enigo (blocking)");
    let result = tokio::task::block_in_place(|| -> Result<(), String> {
        let mut enigo = Enigo::new();
        
        // Type the text
        bprintln!(dev: "💻 INPUT: Typing text of length {}", text.len());
        enigo.key_sequence(text);
        bprintln!(dev: "💻 INPUT: Finished typing text");
        
        Ok(())
    });
    
    if let Err(e) = &result {
        bprintln!(error: "💻 INPUT: ⚠️ Keyboard text input failed: {}", e);
    } else {
        bprintln!(debug: "💻 INPUT: Keyboard text input completed successfully");
    }
    
    result?;
    
    Ok(format!("Typed text into window '{}'", window_id))
}

/// Send a keyboard shortcut to a window
async fn send_keyboard_shortcut(
    key: &str, 
    modifiers: &[String], 
    window_id: &str
) -> Result<String, String> {
    bprintln!(debug: "💻 INPUT: Sending keyboard shortcut: key='{}', modifiers={:?}, window='{}'", 
             key, modifiers, window_id);
    
    // First activate the window
    activate_window(window_id).await?;
    
    // Small delay to ensure window is active
    bprintln!(debug: "💻 INPUT: Waiting 100ms for window activation to settle");
    sleep(Duration::from_millis(100)).await;
    
    // Use tokio's block_in_place for the Enigo operations
    bprintln!(debug: "💻 INPUT: Executing keyboard shortcut with Enigo (blocking)");
    let result = tokio::task::block_in_place(|| -> Result<(), String> {
        let mut enigo = Enigo::new();
        
        // Hold down modifier keys
        bprintln!(dev: "💻 INPUT: Holding down {} modifier keys", modifiers.len());
        for modifier in modifiers {
            if let Some(m_key) = parse_modifier(modifier) {
                bprintln!(dev: "💻 INPUT: Pressing modifier: {}", modifier);
                enigo.key_down(m_key);
            } else {
                bprintln!(warn: "💻 INPUT: ⚠️ Unknown modifier key: {}", modifier);
            }
        }
        
        // Press and release the main key
        bprintln!(dev: "💻 INPUT: Processing main key: '{}'", key);
        if let Some(e_key) = parse_key(key) {
            // For special keys
            bprintln!(dev: "💻 INPUT: Pressing special key: {:?}", e_key);
            enigo.key_click(e_key);
        } else if key.len() == 1 {
            // For regular single character keys
            let c = key.chars().next().unwrap();
            bprintln!(dev: "💻 INPUT: Pressing single character key: '{}'", c);
            enigo.key_sequence(&c.to_string());
        } else {
            // For strings (typed out character by character)
            bprintln!(dev: "💻 INPUT: Typing key as sequence: '{}'", key);
            enigo.key_sequence(key);
        }
        
        // Release modifier keys in reverse order
        bprintln!(dev: "💻 INPUT: Releasing modifier keys in reverse order");
        for modifier in modifiers.iter().rev() {
            if let Some(m_key) = parse_modifier(modifier) {
                bprintln!(dev: "💻 INPUT: Releasing modifier: {}", modifier);
                enigo.key_up(m_key);
            }
        }
        
        Ok(())
    });
    
    if let Err(e) = &result {
        bprintln!(error: "💻 INPUT: ⚠️ Keyboard shortcut failed: {}", e);
    } else {
        bprintln!(debug: "💻 INPUT: Keyboard shortcut completed successfully");
    }
    
    result?;
    
    Ok(format!("Sent keyboard shortcut to window '{}'", window_id))
}

/// Execute a sequence of actions with the given window ID
async fn execute_action_sequence(actions: &[InputAction], window_id: &str) -> ToolResult {
    let mut results = Vec::new();
    
    for (index, action) in actions.iter().enumerate() {
        // Execute the action
        let result = match action {
            InputAction::Click { x, y, button, double } => {
                send_mouse_click(*x, *y, window_id, *button, *double).await
            },
            InputAction::Type { text } => {
                send_keyboard_text(text, window_id).await
            },
            InputAction::KeyPress { key, modifiers } => {
                send_keyboard_shortcut(key, modifiers, window_id).await
            },
            InputAction::Wait { ms } => {
                sleep(Duration::from_millis(*ms)).await;
                Ok(format!("Waited for {}ms", ms))
            }
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
                assert_eq!(button, MouseButtonType::Left);
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
                assert_eq!(button, MouseButtonType::Right);
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
    fn test_parse_key() {
        assert_eq!(parse_key("return"), Some(Key::Return));
        assert_eq!(parse_key("enter"), Some(Key::Return));
        assert_eq!(parse_key("escape"), Some(Key::Escape));
        assert_eq!(parse_key("esc"), Some(Key::Escape));
        assert_eq!(parse_key("a"), None); // Regular key, not special
    }

    #[test]
    fn test_parse_modifier() {
        assert_eq!(parse_modifier("command"), Some(Key::Meta));
        assert_eq!(parse_modifier("cmd"), Some(Key::Meta));
        assert_eq!(parse_modifier("shift"), Some(Key::Shift));
        assert_eq!(parse_modifier("alt"), Some(Key::Alt));
        assert_eq!(parse_modifier("control"), Some(Key::Control));
        assert_eq!(parse_modifier("ctrl"), Some(Key::Control));
        assert_eq!(parse_modifier("unknown"), None);
    }
}