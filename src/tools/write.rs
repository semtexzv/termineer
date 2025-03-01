use std::fs;
use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};

pub fn execute_write(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse the filename from args
    let filename = args.trim();
    
    // Validate filename
    if filename.is_empty() {
        let error_msg = "Write tool requires a filename as an argument".to_string();
        
        if !silent_mode {
            println!("{}❌ Error:{} {}", 
                FORMAT_BOLD, FORMAT_RESET, error_msg);
        }
        
        return ToolResult {
            success: false,
            agent_output: error_msg,
        };
    }

    // Use the entire body as content
    let content = body;

    // Write the file
    match fs::write(filename, content) {
        Ok(_) => {
            // Get content details
            let line_count = content.lines().count();
            
            // Direct output to console if not in silent mode
            if !silent_mode {
                // Get a brief preview (first 2 lines of content)
                let preview_lines = content.lines()
                    .take(2)
                    .collect::<Vec<&str>>()
                    .join("\n");
                    
                let preview = if !preview_lines.is_empty() {
                    format!("\n{}{}{}", FORMAT_GRAY, preview_lines, FORMAT_RESET)
                } else {
                    "".to_string()
                };
                
                println!("{}✍️ Write: {} ({} lines){}{}", 
                    FORMAT_BOLD, 
                    filename, 
                    line_count, 
                    FORMAT_RESET,
                    preview
                );
            }
            
            // More detailed output for the agent including line count
            let agent_output = format!(
                "Successfully wrote to file '{}' ({} lines, line range: 1-{})",
                filename, 
                line_count,
                line_count
            );
            
            ToolResult {
                success: true,
                agent_output,
            }
        },
        Err(e) => {
            let error_msg = format!("Error writing to file '{}': {}", filename, e);
            
            if !silent_mode {
                println!("{}❌ Error:{} {}", 
                    FORMAT_BOLD, FORMAT_RESET, error_msg);
            }
            
            ToolResult {
                success: false,
                agent_output: error_msg,
            }
        },
    }
}