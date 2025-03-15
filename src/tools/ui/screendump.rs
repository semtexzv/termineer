//! Screendump tool module
//!
//! This tool allows extracting UI structure from applications

use crate::tools::ToolResult;

/// Commands supported by the screendump tool
#[derive(Debug, Clone)]
pub enum ScreendumpCommand {
    /// List all windows with identifiers
    ListWindows,
    /// Capture details for a specific window
    WindowDetails(String),
}

/// Parse the command arguments
pub fn parse_command(args: &str) -> ScreendumpCommand {
    let args = args.trim();

    if args.is_empty() {
        return ScreendumpCommand::ListWindows;
    }

    let parts: Vec<&str> = args.split_whitespace().collect();
    
    if parts.is_empty() {
        return ScreendumpCommand::ListWindows;
    }

    match parts[0].to_lowercase().as_str() {
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

/// Execute the screendump tool
pub async fn execute_screendump(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Get platform
    let platform = std::env::consts::OS;

    // Route to platform-specific implementation
    match platform {
        #[cfg(target_os = "macos")]
        "macos" => crate::tools::ui::macos::screendump::execute_macos_screendump(args, body, silent_mode).await,
        
        #[cfg(target_os = "windows")]
        "windows" => crate::tools::ui::windows::screendump::execute_windows_screendump(args, body, silent_mode).await,
        
        #[cfg(target_os = "linux")]
        "linux" => crate::tools::ui::linux::screendump::execute_linux_screendump(args, body, silent_mode).await,
        
        _ => ToolResult::error(format!("Screendump tool not implemented for {} platform", platform))
    }
}

/// Public function to get a window's rectangle by ID
/// 
/// Returns a tuple of (app_name, window_title, x, y, width, height)
pub fn get_window_rect(window_id: &str) -> Result<(String, String, i32, i32, i32, i32), String> {
    // Get platform
    let platform = std::env::consts::OS;
    
    // Route to platform-specific implementation
    match platform {
        #[cfg(target_os = "macos")]
        "macos" => crate::tools::ui::macos::screendump::get_macos_window_rect(window_id),
        
        #[cfg(target_os = "windows")]
        "windows" => crate::tools::ui::windows::screendump::get_windows_window_rect(window_id),
        
        #[cfg(target_os = "linux")]
        "linux" => crate::tools::ui::linux::screendump::get_linux_window_rect(window_id),
        
        _ => Err(format!("Window rect retrieval not implemented for {} platform", platform))
    }
}