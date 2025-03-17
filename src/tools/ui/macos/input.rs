//! macOS implementation of the input tool
//!
//! This module provides macOS-specific implementation for sending
//! mouse and keyboard inputs using macOS APIs.

use crate::tools::ui::input::{InputAction, InputCommand, MouseButtonType};
use crate::tools::ui::screendump;
use crate::tools::ToolResult;
use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::time::Duration;
use tokio::time::sleep;

/// Execute the macOS input tool with the parsed command
pub async fn execute_macos_input(command: InputCommand, silent_mode: bool) -> ToolResult {
    if !silent_mode {
        match &command {
            InputCommand::Click {
                x,
                y,
                window_id,
                button,
                double,
            } => {
                let button_str = match button {
                    MouseButtonType::Left => "left",
                    MouseButtonType::Right => "right",
                    MouseButtonType::Middle => "middle",
                };
                let double_str = if *double { "double " } else { "" };
                crate::bprintln!(
                    "🖱️ Sending {}{}click at ({},{}) to window '{}'...",
                    double_str,
                    button_str,
                    x,
                    y,
                    window_id
                );
            }
            InputCommand::Type { text, window_id } => {
                crate::bprintln!("⌨️ Typing text to window '{}'...", window_id);
                crate::bprintln!(dev: "💻 INPUT: Will type text of length {}: '{}'", 
                                text.len(), 
                                if text.len() > 50 { format!("{}...", &text[..50]) } else { text.clone() });
            }
            InputCommand::KeyPress {
                key,
                modifiers,
                window_id,
            } => {
                crate::bprintln!(
                    "⌨️ Sending key {} with modifiers {} to window '{}'...",
                    key,
                    modifiers.join("+"),
                    window_id
                );
            }
            InputCommand::Sequence { actions, window_id } => {
                crate::bprintln!(
                    "🤖 Executing {} input actions on window '{}'...",
                    actions.len(),
                    window_id
                );
                for (i, action) in actions.iter().enumerate() {
                    crate::bprintln!(dev: "💻 INPUT: Action {}: {:?}", i+1, action);
                }
            }
        }
    }

    match command {
        InputCommand::Click {
            x,
            y,
            window_id,
            button,
            double,
        } => match send_mouse_click(x, y, &window_id, button, double).await {
            Ok(msg) => ToolResult::success(msg),
            Err(err) => ToolResult::error(err),
        },
        InputCommand::Type { text, window_id } => {
            match send_keyboard_text(&text, &window_id).await {
                Ok(msg) => ToolResult::success(msg),
                Err(err) => ToolResult::error(err),
            }
        }
        InputCommand::KeyPress {
            key,
            modifiers,
            window_id,
        } => match send_keyboard_shortcut(&key, &modifiers, &window_id).await {
            Ok(msg) => ToolResult::success(msg),
            Err(err) => ToolResult::error(err),
        },
        InputCommand::Sequence { actions, window_id } => {
            execute_action_sequence(&actions, &window_id).await
        }
    }
}

/// Convert MouseButtonType to Enigo's MouseButton
fn to_enigo_button(button: MouseButtonType) -> Button {
    match button {
        MouseButtonType::Left => Button::Left,
        MouseButtonType::Right => Button::Right,
        MouseButtonType::Middle => Button::Middle,
    }
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
    crate::bprintln!(dev: "💻 INPUT: Getting position for window '{}'", window_id);

    // Get window information from screendump module
    let result = screendump::get_window_rect(window_id);

    match &result {
        Ok((app, title, x, y, width, height)) => {
            crate::bprintln!(dev: "💻 INPUT: Window '{}' found: app='{}', title='{}', position=({},{}), size={}x{}", 
                     window_id, app, title, x, y, width, height);
        }
        Err(e) => {
            crate::bprintln!(warn: "💻 INPUT: ⚠️ Failed to get window position: {}", e);
        }
    }

    let (_, _, x, y, _, _) = result.map_err(|e| format!("Failed to get window position: {}", e))?;

    Ok((x, y))
}

/// Activate a window by its ID
async fn activate_window(window_id: &str) -> Result<(), String> {
    crate::bprintln!(dev: "💻 INPUT: Activating window '{}'", window_id);

    // First check if we can get the window rect (validates it exists)
    match screendump::get_window_rect(window_id) {
        Ok(_) => crate::bprintln!(dev: "💻 INPUT: Window '{}' found, will activate", window_id),
        Err(e) => {
            let error = format!("Failed to find window: {}", e);
            crate::bprintln!(error: "💻 INPUT: ⚠️ {}", error);
            return Err(error);
        }
    }

    let parts: Vec<&str> = window_id.split(':').collect();
    if parts.is_empty() {
        let error = "Invalid window ID format".to_string();
        crate::bprintln!(error: "💻 INPUT: ⚠️ {}", error);
        return Err(error);
    }

    let app_name = parts[0];
    crate::bprintln!(dev: "💻 INPUT: Using osascript to activate application '{}'", app_name);

    // Use AppleScript to activate the application
    let script = format!("tell application \"{}\" to activate", app_name);
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| {
            let error = format!("Failed to execute osascript: {}", e);
            crate::bprintln!(error: "💻 INPUT: ⚠️ {}", error);
            error
        })?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let error_msg = format!("Failed to activate window: {}", error);
        crate::bprintln!(error: "💻 INPUT: ⚠️ {}", error_msg);
        return Err(error_msg);
    }

    crate::bprintln!(dev: "💻 INPUT: Successfully activated '{}', waiting for activation to complete", app_name);
    // Wait for the activation to complete
    sleep(Duration::from_millis(200)).await;

    crate::bprintln!(dev: "💻 INPUT: Window activation completed for '{}'", window_id);
    Ok(())
}

/// Send a mouse click at the specified coordinates
async fn send_mouse_click(
    x: i32,
    y: i32,
    window_id: &str,
    button: MouseButtonType,
    double: bool,
) -> Result<String, String> {
    crate::bprintln!(dev: "💻 INPUT: Sending mouse click: x={}, y={}, window='{}', button={:?}, double={}", 
              x, y, window_id, button, double);

    // First activate the window
    activate_window(window_id).await?;

    // Get the window position
    let (win_x, win_y) = get_window_position(window_id).await?;

    // Calculate absolute screen coordinates
    let abs_x = win_x + x;
    let abs_y = win_y + y;
    crate::bprintln!(dev: "💻 INPUT: Calculated absolute coordinates: ({}, {})", abs_x, abs_y);

    crate::bprintln!(dev: "💻 INPUT: Executing mouse click with Enigo (blocking)");
    let result = tokio::task::block_in_place(|| -> Result<(), String> {
        // Create a new Enigo instance
        let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();

        // Move to the position
        Mouse::move_mouse(&mut enigo, abs_x, abs_y, Coordinate::Abs).map_err(|e| e.to_string())?;

        // Perform the click
        let enigo_button = to_enigo_button(button);

        // Single or double click
        Mouse::button(&mut enigo, enigo_button, Direction::Press).map_err(|e| e.to_string())?;
        Mouse::button(&mut enigo, enigo_button, Direction::Release).map_err(|e| e.to_string())?;

        if double {
            // Small pause between clicks for double-click
            std::thread::sleep(Duration::from_millis(10));
            Mouse::button(&mut enigo, enigo_button, Direction::Press).map_err(|e| e.to_string())?;
            Mouse::button(&mut enigo, enigo_button, Direction::Release)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    });

    if let Err(e) = &result {
        crate::bprintln!(error: "💻 INPUT: ⚠️ Mouse click failed: {}", e);
    } else {
        crate::bprintln!(dev: "💻 INPUT: Mouse click completed successfully");
    }

    result?;

    Ok(format!(
        "Clicked at coordinates ({}, {}) in window '{}'",
        x, y, window_id
    ))
}

/// Type text into a window
async fn send_keyboard_text(text: &str, window_id: &str) -> Result<String, String> {
    crate::bprintln!(dev: "💻 INPUT: Sending keyboard text to window '{}'", window_id);
    if text.len() > 100 {
        crate::bprintln!(dev: "💻 INPUT: Text content (first 100 chars): '{}'...", &text[..100]);
    } else {
        crate::bprintln!(dev: "💻 INPUT: Text content: '{}'", text);
    }

    // First activate the window
    activate_window(window_id).await?;

    // Small delay to ensure window is active
    crate::bprintln!(dev: "💻 INPUT: Waiting 100ms for window activation to settle");
    sleep(Duration::from_millis(100)).await;

    // Use tokio's block_in_place for the Enigo operations
    crate::bprintln!(dev: "💻 INPUT: Executing keyboard input with Enigo (blocking)");
    let result = tokio::task::block_in_place(|| -> Result<(), String> {
        let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();

        // Type the text using the Keyboard trait
        Keyboard::text(&mut enigo, text).map_err(|e| e.to_string())?;

        Ok(())
    });

    if let Err(e) = &result {
        crate::bprintln!(error: "💻 INPUT: ⚠️ Keyboard text input failed: {}", e);
    } else {
        crate::bprintln!(dev: "💻 INPUT: Keyboard text input completed successfully");
    }

    result?;

    Ok(format!("Typed text into window '{}'", window_id))
}

/// Send a keyboard shortcut to a window
async fn send_keyboard_shortcut(
    key: &str,
    modifiers: &[String],
    window_id: &str,
) -> Result<String, String> {
    crate::bprintln!(dev: "💻 INPUT: Sending keyboard shortcut: key='{}', modifiers={:?}, window='{}'", 
             key, modifiers, window_id);

    // First activate the window
    activate_window(window_id).await?;

    // Small delay to ensure window is active
    crate::bprintln!(dev: "💻 INPUT: Waiting 100ms for window activation to settle");
    sleep(Duration::from_millis(100)).await;

    // Use tokio's block_in_place for the Enigo operations
    crate::bprintln!(dev: "💻 INPUT: Executing keyboard shortcut with Enigo (blocking)");
    let result = tokio::task::block_in_place(|| -> Result<(), String> {
        let mut enigo = Enigo::new(&enigo::Settings::default()).unwrap();

        // Hold down modifier keys
        for modifier in modifiers {
            if let Some(m_key) = parse_modifier(modifier) {
                Keyboard::key(&mut enigo, m_key, Direction::Press).map_err(|e| e.to_string())?;
            } else {
                crate::bprintln!(warn: "💻 INPUT: ⚠️ Unknown modifier key: {}", modifier);
            }
        }

        // Press and release the main key
        if let Some(e_key) = parse_key(key) {
            // For special keys
            Keyboard::key(&mut enigo, e_key, Direction::Press).map_err(|e| e.to_string())?;
            Keyboard::key(&mut enigo, e_key, Direction::Release).map_err(|e| e.to_string())?;
        } else if key.len() == 1 {
            // For regular single character keys
            let c = key.chars().next().unwrap();
            Keyboard::text(&mut enigo, &c.to_string()).map_err(|e| e.to_string())?;
        } else {
            // For strings (typed out character by character)
            Keyboard::text(&mut enigo, key).map_err(|e| e.to_string())?;
        }

        // Release modifier keys in reverse order
        for modifier in modifiers.iter().rev() {
            if let Some(m_key) = parse_modifier(modifier) {
                Keyboard::key(&mut enigo, m_key, Direction::Release).map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    });

    if let Err(e) = &result {
        crate::bprintln!(error: "💻 INPUT: ⚠️ Keyboard shortcut failed: {}", e);
    } else {
        crate::bprintln!(dev: "💻 INPUT: Keyboard shortcut completed successfully");
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
            InputAction::Click {
                x,
                y,
                button,
                double,
            } => send_mouse_click(*x, *y, window_id, *button, *double).await,
            InputAction::Type { text } => send_keyboard_text(text, window_id).await,
            InputAction::KeyPress { key, modifiers } => {
                send_keyboard_shortcut(key, modifiers, window_id).await
            }
            InputAction::Wait { ms } => {
                sleep(Duration::from_millis(*ms)).await;
                Ok(format!("Waited for {}ms", ms))
            }
        };

        // Process the result
        match result {
            Ok(msg) => {
                results.push(format!("Action {}: {}", index + 1, msg));
            }
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
