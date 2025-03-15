//! Screenshot tool module
//!
//! This tool allows capturing screenshots of entire screens or specific windows

use crate::tools::ToolResult;

/// Execute the screenshot tool
pub async fn execute_screenshot(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Get platform
    let platform = std::env::consts::OS;

    // Route to platform-specific implementation
    match platform {
        #[cfg(target_os = "macos")]
        "macos" => crate::tools::ui::macos::screenshot::execute_macos_screenshot(args, body, silent_mode).await,
        
        #[cfg(target_os = "windows")]
        "windows" => crate::tools::ui::windows::screenshot::execute_windows_screenshot(args, body, silent_mode).await,
        
        #[cfg(target_os = "linux")]
        "linux" => crate::tools::ui::linux::screenshot::execute_linux_screenshot(args, body, silent_mode).await,
        
        _ => ToolResult::error(format!("Screenshot tool not implemented for {} platform", platform))
    }
}