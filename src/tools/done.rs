use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};

pub fn execute_done(args: &str, body: &str) -> ToolResult {
    // Use body as the summary if provided, otherwise use args, and if both are empty, use default text
    let summary = if !body.trim().is_empty() {
        body.trim()
    } else if !args.trim().is_empty() {
        args.trim()
    } else {
        "Task completed successfully."
    };
    
    let preview = format!("\n{}{}{}", FORMAT_GRAY, summary, FORMAT_RESET);
    
    let user_output = format!("{}✅ Task Complete{}{}", FORMAT_BOLD, FORMAT_RESET, preview);
    
    // Pass the summary directly to the agent without extraneous markers
    let agent_output = summary.to_string();
    
    ToolResult {
        success: true,
        user_output,
        agent_output,
    }
}