//! Wait tool for agents to pause until they receive a message
//!
//! This tool allows agents to signal they are ready to wait for messages
//! from users or other agents, pausing their operation until input is received.

use crate::tools::ToolResult;

/// Execute the wait tool
pub fn execute_wait(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Get the reason for waiting from args (optional) or body
    let wait_reason = if !args.trim().is_empty() {
        args.trim().to_string()
    } else if !body.trim().is_empty() {
        body.trim().to_string()
    } else {
        "No specific reason provided".to_string()
    };

    if !silent_mode {
        crate::btool_println!(
            "wait",
            "{}⏸️ Waiting:{} Agent will wait for messages: {}",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            wait_reason
        );
    }
    
    // Return a tool result that puts the agent in waiting state
    ToolResult::wait(wait_reason)
}