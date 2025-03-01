use std::fs;
use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};

pub fn execute_write(args: &str, body: &str) -> ToolResult {
    // Parse the filename from args
    let filename = args.trim();
    
    // Validate filename
    if filename.is_empty() {
        let error_msg = "Write tool requires a filename as an argument".to_string();
        return ToolResult {
            success: false,
            user_output: error_msg.clone(),
            agent_output: error_msg,
        };
    }

    // Use the entire body as content
    let content = body;

    // Write the file
    match fs::write(filename, content) {
        Ok(_) => {
            // Create a more concise user message with a preview of the written content
            // Get a brief preview (first 2 lines of content)
            let preview_lines = content.lines()
                .take(2)
                .collect::<Vec<&str>>()
                .join("\n");
                
            let line_count = content.lines().count();
            
            let preview = if !preview_lines.is_empty() {
                format!("\n{}{}{}", FORMAT_GRAY, preview_lines, FORMAT_RESET)
            } else {
                "".to_string()
            };
            
            let user_output = format!("{}✍Write: {} ({} lines){}{}", 
                FORMAT_BOLD, 
                filename, 
                line_count, 
                FORMAT_RESET,
                preview
            );
            
            // More detailed output for the agent including line count
            let agent_output = format!(
                "Successfully wrote to file '{}' ({} lines, line range: 1-{})",
                filename, 
                line_count,
                line_count
            );
            
            ToolResult {
                success: true,
                user_output,
                agent_output,
            }
        },
        Err(e) => {
            let error_msg = format!("Error writing to file '{}': {}", filename, e);
            ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            }
        },
    }
}