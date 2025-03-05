use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;
use tokio::fs;

pub async fn execute_write(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse the filename from args
    let filename = args.trim();

    // Validate filename
    if filename.is_empty() {
        let error_msg = "Write tool requires a filename as an argument".to_string();

        if !silent_mode {
            // Use buffer-based printing
            crate::berror_println!("{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    // Validate path to prevent path traversal attacks
    let validated_path = match crate::tools::path_utils::validate_path(filename) {
        Ok(path) => path,
        Err(e) => {
            let error_msg = format!("Security error for file '{}': {}", filename, e);

            if !silent_mode {
                // Use buffer-based printing
                crate::berror_println!("{}", error_msg);
            }

            return ToolResult::error(error_msg);
        }
    };

    // Use the entire body as content
    let content = body;
    
    // Get a safe display path for output messages
    let safe_display_path = validated_path.to_string_lossy();

    // Write the file using async I/O with validated path
    match fs::write(&validated_path, content).await {
        Ok(_) => {
            // Get content details
            let line_count = content.lines().count();

            // Direct output to console if not in silent mode
            if !silent_mode {
                // Get a brief preview (first 2 lines of content)
                let preview_lines = content.lines().take(2).collect::<Vec<&str>>().join("\n");

                // Use buffer-based printing
                if !preview_lines.is_empty() {
                    crate::btool_println!(
                        "write",
                        "{}✍️ Write: {} ({} lines){}\n{}{}{}",
                        FORMAT_BOLD,
                        safe_display_path,
                        line_count,
                        FORMAT_RESET,
                        FORMAT_GRAY,
                        preview_lines,
                        FORMAT_RESET
                    );
                } else {
                    crate::btool_println!(
                        "write",
                        "{}✍️ Write: {} ({} lines){}",
                        FORMAT_BOLD,
                        safe_display_path,
                        line_count,
                        FORMAT_RESET
                    );
                }
            }

            // More detailed output for the agent including line count
            let agent_output = format!(
                "Successfully wrote to file '{}' ({} lines, line range: 1-{})",
                safe_display_path, line_count, line_count
            );

            ToolResult::success(agent_output)
        }
        Err(e) => {
            if !silent_mode {
                // Use buffer-based printing with direct error message
                crate::berror_println!("Error writing to file '{}': {}", safe_display_path, e);
            }

            let error_msg = format!("Error writing to file '{}': {}", safe_display_path, e);

            ToolResult::error(error_msg)
        }
    }
}
