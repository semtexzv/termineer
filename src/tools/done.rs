use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;

// We keep the done tool non-async since it doesn't need to wait for any async operations
// Other tools call this directly without awaiting
pub fn execute_done(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Use body as the summary if provided, otherwise use args, and if both are empty, use default text
    let summary = if !body.trim().is_empty() {
        body.trim()
    } else if !args.trim().is_empty() {
        args.trim()
    } else {
        "Task completed successfully."
    };

    // Direct output to console if not in silent mode
    if !silent_mode {
        // Use buffer-based printing directly
        crate::btool_println!("done", "{}✅ Task Complete{}", FORMAT_BOLD, FORMAT_RESET);
        crate::bprintln!("{}{}{}", FORMAT_GRAY, summary, FORMAT_RESET);
    }

    // Pass the summary directly to the agent without extraneous markers
    let agent_output = summary.to_string();

    ToolResult {
        success: true,
        agent_output,
    }
}
