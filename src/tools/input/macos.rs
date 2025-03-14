//! macOS implementation of the input tool
//!
//! Uses Accessibility API, CoreGraphics and AppleScript to send
//! mouse and keyboard input to specific windows.

use crate::tools::screendump;
use crate::tools::input::MouseButton;

use core_graphics::event::{
    CGEvent, CGEventType, CGEventTapLocation, CGMouseButton
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

use std::process::Command;
use tokio::time::sleep;
use std::time::Duration;
use accessibility_sys_ng::AXIsProcessTrusted;
use std::sync::Once;

// Single-character keyboard codes
static KEYBOARD_CODES: phf::Map<&'static str, u16> = phf::phf_map! {
    "a" => 0x00,
    "s" => 0x01,
    "d" => 0x02,
    "f" => 0x03,
    "h" => 0x04,
    "g" => 0x05,
    "z" => 0x06,
    "x" => 0x07,
    "c" => 0x08,
    "v" => 0x09,
    "b" => 0x0B,
    "q" => 0x0C,
    "w" => 0x0D,
    "e" => 0x0E,
    "r" => 0x0F,
    "y" => 0x10,
    "t" => 0x11,
    "1" => 0x12,
    "2" => 0x13,
    "3" => 0x14,
    "4" => 0x15,
    "6" => 0x16,
    "5" => 0x17,
    "=" => 0x18,
    "9" => 0x19,
    "7" => 0x1A,
    "-" => 0x1B,
    "8" => 0x1C,
    "0" => 0x1D,
    "]" => 0x1E,
    "o" => 0x1F,
    "u" => 0x20,
    "[" => 0x21,
    "i" => 0x22,
    "p" => 0x23,
    "return" => 0x24,
    "l" => 0x25,
    "j" => 0x26,
    "'" => 0x27,
    "k" => 0x28,
    ";" => 0x29,
    "\\" => 0x2A,
    "," => 0x2B,
    "/" => 0x2C,
    "n" => 0x2D,
    "m" => 0x2E,
    "." => 0x2F,
    "tab" => 0x30,
    "space" => 0x31,
    "`" => 0x32,
    "delete" => 0x33,
    "escape" => 0x35,
    "command" => 0x37,
    "shift" => 0x38,
    "capslock" => 0x39,
    "option" => 0x3A,
    "control" => 0x3B,
    "rightshift" => 0x3C,
    "rightoption" => 0x3D,
    "rightcontrol" => 0x3E,
    "fn" => 0x3F,
    "f17" => 0x40,
    "keypad." => 0x41,
    "keypad*" => 0x43,
    "keypad+" => 0x45,
    "keypadclear" => 0x47,
    "volumeup" => 0x48,
    "volumedown" => 0x49,
    "mute" => 0x4A,
    "keypad/" => 0x4B,
    "keypadenter" => 0x4C,
    "keypad-" => 0x4E,
    "f18" => 0x4F,
    "f19" => 0x50,
    "keypad=" => 0x51,
    "keypad0" => 0x52,
    "keypad1" => 0x53,
    "keypad2" => 0x54,
    "keypad3" => 0x55,
    "keypad4" => 0x56,
    "keypad5" => 0x57,
    "keypad6" => 0x58,
    "keypad7" => 0x59,
    "f20" => 0x5A,
    "keypad8" => 0x5B,
    "keypad9" => 0x5C,
    "f5" => 0x60,
    "f6" => 0x61,
    "f7" => 0x62,
    "f3" => 0x63,
    "f8" => 0x64,
    "f9" => 0x65,
    "f11" => 0x67,
    "f13" => 0x69,
    "f16" => 0x6A,
    "f14" => 0x6B,
    "f10" => 0x6D,
    "f12" => 0x6F,
    "f15" => 0x71,
    "home" => 0x73,
    "pageup" => 0x74,
    "forwarddelete" => 0x75,
    "f4" => 0x76,
    "end" => 0x77,
    "f2" => 0x78,
    "pagedown" => 0x79,
    "f1" => 0x7A,
    "leftarrow" => 0x7B,
    "rightarrow" => 0x7C,
    "downarrow" => 0x7D,
    "uparrow" => 0x7E,
};

// Initialize once flag for accessibility check
static INIT_ACCESSIBILITY: Once = Once::new();

/// Check if accessibility permissions are granted
fn check_accessibility() -> Result<(), String> {
    // Only check once per process
    let mut has_permissions = false;
    
    INIT_ACCESSIBILITY.call_once(|| {
        unsafe {
            has_permissions = AXIsProcessTrusted();
        }
    });
    
    if has_permissions {
        Ok(())
    } else {
        Err("Accessibility access is not enabled for this application. Please enable it in System Preferences > Security & Privacy > Privacy > Accessibility".to_string())
    }
}

/// Get the window position and size from its ID
async fn get_window_bounds(window_id: &str) -> Result<(i32, i32, i32, i32), String> {
    // Check accessibility permissions
    check_accessibility()?;
    
    // Get window information from screendump module
    let (_, _, x, y, width, height) = screendump::get_window_rect(window_id)?;
    
    Ok((x, y, width, height))
}

/// Convert string modifiers to virtual key codes
#[allow(dead_code)]
fn get_modifier_flags(modifiers: &[String]) -> u64 {
    let mut flags = 0;
    
    for modifier in modifiers {
        match modifier.to_lowercase().as_str() {
            "cmd" | "command" => flags |= 1 << 8,  // kCGEventFlagMaskCommand
            "shift" => flags |= 1 << 9,           // kCGEventFlagMaskShift
            "alt" | "option" => flags |= 1 << 11,  // kCGEventFlagMaskAlternate
            "ctrl" | "control" => flags |= 1 << 12, // kCGEventFlagMaskControl
            _ => {}
        }
    }
    
    flags
}

/// Send a mouse click at the specified coordinates
pub async fn send_mouse_click(
    x: i32, 
    y: i32, 
    window_id: &str, 
    button: MouseButton,
    double: bool
) -> Result<String, String> {
    // Check accessibility permissions
    check_accessibility()?;
    
    // First, activate the window
    activate_window(window_id).await?;
    
    // Then, get window bounds
    let (win_x, win_y, _, _) = get_window_bounds(window_id).await?;
    
    // Calculate absolute screen coordinates
    let abs_x = win_x + x;
    let abs_y = win_y + y;
    
    // Get CoreGraphics mouse button
    let cg_button = match button {
        MouseButton::Left => CGMouseButton::Left,
        MouseButton::Right => CGMouseButton::Right,
        MouseButton::Middle => CGMouseButton::Center,
    };
    
    // Small delay to ensure window is active before clicking
    sleep(Duration::from_millis(100)).await;
    
    // Create event source and send events - use tokio's block_in_place for synchronous calls
    tokio::task::block_in_place(move || -> Result<(), String> {
        // Create event source
        let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState)
            .map_err(|_| "Failed to create event source".to_string())?;
        
        // Create mouse events
        let event_down = CGEvent::new_mouse_event(
            source.clone(),
            match button {
                MouseButton::Left => CGEventType::LeftMouseDown,
                MouseButton::Right => CGEventType::RightMouseDown,
                MouseButton::Middle => CGEventType::OtherMouseDown,
            },
            CGPoint::new(abs_x as f64, abs_y as f64),
            cg_button,
        ).map_err(|_| "Failed to create mouse down event".to_string())?;
        
        let event_up = CGEvent::new_mouse_event(
            source.clone(),
            match button {
                MouseButton::Left => CGEventType::LeftMouseUp,
                MouseButton::Right => CGEventType::RightMouseUp,
                MouseButton::Middle => CGEventType::OtherMouseUp,
            },
            CGPoint::new(abs_x as f64, abs_y as f64),
            cg_button,
        ).map_err(|_| "Failed to create mouse up event".to_string())?;
        
        // Post the events
        event_down.post(CGEventTapLocation::HID);
        std::thread::sleep(Duration::from_millis(10));
        event_up.post(CGEventTapLocation::HID);
        
        // If double-click, send another click
        if double {
            std::thread::sleep(Duration::from_millis(10));
            event_down.post(CGEventTapLocation::HID);
            std::thread::sleep(Duration::from_millis(10));
            event_up.post(CGEventTapLocation::HID);
        }
        
        Ok(())
    })?;
    
    Ok(format!("Clicked at coordinates ({}, {}) in window '{}'", x, y, window_id))
}

/// Activate a window by ID
async fn activate_window(window_id: &str) -> Result<(), String> {
    // Check accessibility permissions
    check_accessibility()?;
    
    // Parse window ID to get app name
    let parts: Vec<&str> = window_id.split(':').collect();
    
    if parts.is_empty() {
        return Err("Invalid window ID format".to_string());
    }
    
    let app_name = parts[0];
    
    // Create AppleScript to activate the application
    let applescript = format!(
        r#"
        tell application "{}"
            activate
        end tell
        "#,
        app_name
    );
    
    // Run AppleScript
    let output = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to activate window: {}", error));
    }
    
    // Wait a moment for activation to complete
    sleep(Duration::from_millis(200)).await;
    
    Ok(())
}

/// Type text into a window
pub async fn send_keyboard_text(text: &str, window_id: &str) -> Result<String, String> {
    // Check accessibility permissions
    check_accessibility()?;
    
    // Activate the window first
    activate_window(window_id).await?;
    
    // Create AppleScript to type text
    let applescript = format!(
        r#"
        tell application "System Events"
            keystroke "{}"
        end tell
        "#,
        text.replace("\"", "\\\"") // Escape quotes for AppleScript
    );
    
    // Run AppleScript
    let output = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to type text: {}", error));
    }
    
    Ok(format!("Typed text into window '{}'", window_id))
}

/// Send a keyboard shortcut to a window
pub async fn send_keyboard_shortcut(
    key: &str, 
    modifiers: &[String], 
    window_id: &str
) -> Result<String, String> {
    // Check accessibility permissions
    check_accessibility()?;
    
    // Activate the window first
    activate_window(window_id).await?;
    
    // Convert modifiers to AppleScript format
    let applescript_modifiers = modifiers
        .iter()
        .map(|m| match m.to_lowercase().as_str() {
            "cmd" | "command" => "command down",
            "shift" => "shift down",
            "alt" | "option" => "option down",
            "ctrl" | "control" => "control down",
            _ => "",
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    
    // Create AppleScript to send key with modifiers
    let applescript = if applescript_modifiers.is_empty() {
        format!(
            r#"
            tell application "System Events"
                key code {}
            end tell
            "#,
            get_key_code(key)
        )
    } else {
        format!(
            r#"
            tell application "System Events"
                key code {} using {{{} }}
            end tell
            "#,
            get_key_code(key),
            applescript_modifiers
        )
    };
    
    // Run AppleScript
    let output = Command::new("osascript")
        .args(["-e", &applescript])
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;
    
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to send keyboard shortcut: {}", error));
    }
    
    Ok(format!("Sent keyboard shortcut to window '{}'", window_id))
}

/// Get the key code for a key
fn get_key_code(key: &str) -> u16 {
    let key_lower = key.to_lowercase();
    
    // Special case for key codes
    if key_lower.starts_with("keycode:") {
        if let Some(code_str) = key_lower.strip_prefix("keycode:") {
            if let Ok(code) = code_str.parse::<u16>() {
                return code;
            }
        }
    }
    
    // Look up in the key code map
    KEYBOARD_CODES.get(key_lower.as_str()).copied().unwrap_or_else(|| {
        // Handle special cases or return a default value
        match key_lower.as_str() {
            "enter" => *KEYBOARD_CODES.get("return").unwrap(),
            "esc" => *KEYBOARD_CODES.get("escape").unwrap(),
            _ => {
                // For single characters, just use the first character
                if key_lower.len() == 1 {
                    if let Some(code) = KEYBOARD_CODES.get(key_lower.as_str()) {
                        return *code;
                    }
                }
                
                // Default to 'a' key code
                *KEYBOARD_CODES.get("a").unwrap()
            }
        }
    })
}