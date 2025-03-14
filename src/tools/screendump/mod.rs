//! Tool for capturing the current UI display using the accessibility crate
//!
//! This tool allows agents to capture the current UI structure as text,
//! providing information about window layout, controls, and hierarchies.

use crate::tools::ToolResult;
use std::env;

#[cfg(target_os = "macos")]
mod macos;


/// Execute the UI dump tool to capture accessibility tree
pub async fn execute_screendump(args: &str, _body: &str, silent_mode: bool) -> ToolResult {
    let platform = env::consts::OS;
    let command = parse_command(args);

    crate::bprintln!(dev: "Screendump tool executing with args: '{}', command: {:?}", args, command);

    if !silent_mode {
        match &command {
            ScreendumpCommand::ListWindows => {
                crate::bprintln!("🔍 Listing all windows on {} platform...", platform);
            },
            ScreendumpCommand::WindowDetails(id) => {
                crate::bprintln!("🔍 Capturing details for window '{}' on {} platform...", id, platform);
            },
            ScreendumpCommand::FullScreen => {
                crate::bprintln!("🔍 Capturing full screen UI hierarchy on {} platform...", platform);
            },
        }
    }

    match platform {
        "macos" => macos::capture_macos_ui(command, silent_mode),
        "windows" => capture_windows_ui(command),
        "linux" => capture_linux_ui(command),
        _ => ToolResult::error(format!("Screendump not implemented for {} platform", platform))
    }
}

/// Commands supported by the screendump tool
#[derive(Debug, Clone)]
enum ScreendumpCommand {
    /// List all windows with identifiers
    ListWindows,
    /// Capture details for a specific window
    WindowDetails(String),
    /// Capture full screen hierarchy (all windows)
    FullScreen,
}

/// Parse the command arguments
fn parse_command(args: &str) -> ScreendumpCommand {
    let args = args.trim();

    if args.is_empty() {
        return ScreendumpCommand::ListWindows;
    }

    let parts: Vec<&str> = args.split_whitespace().collect();

    match parts[0].to_lowercase().as_str() {
        "fullscreen" | "full" => ScreendumpCommand::FullScreen,
        "list" => ScreendumpCommand::ListWindows,
        "window" | "win" => {
            if parts.len() > 1 {
                ScreendumpCommand::WindowDetails(parts[1..].join(" "))
            } else {
                ScreendumpCommand::ListWindows
            }
        },
        id if id.parse::<i32>().is_ok() => ScreendumpCommand::WindowDetails(id.to_string()),
        _ => ScreendumpCommand::WindowDetails(args.to_string())
    }
}

/// Public function to get a window's rectangle by ID
/// 
/// Returns a tuple of (app_name, window_title, x, y, width, height) where:
/// - On macOS: Coordinates use a bottom-left origin system (0,0 at bottom-left)
/// - On Windows: Coordinates use a top-left origin system (0,0 at top-left) [not implemented yet]
/// - On Linux: Coordinates use a top-left origin system (0,0 at top-left) [not implemented yet]
pub fn get_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
    let platform = env::consts::OS;
    crate::bprintln!(dev: "🖥️ SCREENDUMP: Getting window rectangle on {} platform for '{}'", platform, window_id);

    match platform {
        "macos" => {
            crate::bprintln!(dev: "🖥️ SCREENDUMP: Using macOS coordinate system (0,0 at bottom-left)");
            macos::get_macos_window_rect(window_id)
        },
        _ => {
            let error = format!("Window rect retrieval not implemented for {} platform", platform);
            crate::bprintln!(error: "🖥️ SCREENDUMP: {}", error);
            Err(error)
        }
    }
}


fn capture_windows_ui(command: ScreendumpCommand) -> ToolResult {
    let description = match command {
        ScreendumpCommand::ListWindows => "list windows",
        ScreendumpCommand::WindowDetails(ref id) => &format!("window details for '{}'", id),
        ScreendumpCommand::FullScreen => "full screen",
    };

    ToolResult::error(format!("Windows UI capture ({}) not yet implemented", description))
}

fn capture_linux_ui(command: ScreendumpCommand) -> ToolResult {
    let description = match command {
        ScreendumpCommand::ListWindows => "list windows",
        ScreendumpCommand::WindowDetails(ref id) => &format!("window details for '{}'", id),
        ScreendumpCommand::FullScreen => "full screen",
    };

    ToolResult::error(format!("Linux UI capture ({}) not yet implemented", description))
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn test_parse_command() {
        // Test empty string
        assert!(matches!(parse_command(""), ScreendumpCommand::ListWindows));

        // Test fullscreen
        assert!(matches!(parse_command("fullscreen"), ScreendumpCommand::FullScreen));
        assert!(matches!(parse_command("full"), ScreendumpCommand::FullScreen));

        // Test list
        assert!(matches!(parse_command("list"), ScreendumpCommand::ListWindows));

        // Test window command
        match parse_command("window Terminal") {
            ScreendumpCommand::WindowDetails(id) => assert_eq!(id, "Terminal"),
            _ => panic!("Expected WindowDetails"),
        }

        // Test window with no args
        assert!(matches!(parse_command("window"), ScreendumpCommand::ListWindows));

        // Test numeric ID
        match parse_command("1234") {
            ScreendumpCommand::WindowDetails(id) => assert_eq!(id, "1234"),
            _ => panic!("Expected WindowDetails"),
        }

        // Test direct title
        match parse_command("Some Window Title") {
            ScreendumpCommand::WindowDetails(id) => assert_eq!(id, "Some Window Title"),
            _ => panic!("Expected WindowDetails"),
        }
    }
}