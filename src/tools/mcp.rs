//! Dynamic MCP tool handler
//! 
//! This module implements a dynamic approach where any registered MCP server
//! can be directly invoked as a tool by name.

use serde_json::Value;
use crate::mcp::protocol::content::McpContent;
use crate::tools::{AgentStateChange, ToolResult};

/// Execute a dynamic MCP tool
///
/// This handles tool invocations where the tool name is an MCP server name
/// and the first argument is the tool name within that server.
///
/// @param server_name The MCP server name
/// @param args The tool arguments (first positional argument is the tool name)
/// @param body The JSON body for the tool parameters
/// @param silent_mode Whether to suppress console output
pub async fn execute_dynamic_mcp_tool(
    server_name: &str,
    args: &str,
    body: &str,
    silent_mode: bool,
) -> ToolResult {
    // Extract the tool name from args (first positional argument)
    let tool_name = args.trim();
    
    if tool_name.is_empty() {
        return ToolResult::error(format!(
            "MCP tool '{}' is not available. Use a valid MCP tool syntax.",
            server_name
        ));
    }
    
    // Get the provider using the MCP API
    let provider = match crate::mcp::get_provider(server_name) {
        Some(provider) => provider,
        None => {
            if !silent_mode {
                bprintln!(error: "MCP server not found: {}", server_name);
            }

            return ToolResult::error(format!(
                "MCP server '{}' is not available. Use a valid MCP server name.",
                server_name
            ));
        }
    };

    // Parse tool arguments from body
    let arguments: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(err) => {
            if !silent_mode {
                bprintln!(error: "Failed to parse arguments as JSON: {}", err);
            }

            return ToolResult::error(format!(
                "Failed to parse arguments as JSON: {}. Please provide valid JSON parameters.",
                err
            ));
        }
    };

    // Format invocation message similar to the read tool style
    if !silent_mode {
        // Bold invocation message with icon and MCP tool details
        bprintln!(
            tool: "mcp",
            "{}🔌 Executing: {}.{}{}",
            crate::constants::FORMAT_BOLD,
            server_name,
            tool_name,
            crate::constants::FORMAT_RESET
        );
        
        // Gray content - show argument summary in a format similar to read tool preview
        if arguments.is_object() && !arguments.as_object().unwrap().is_empty() {
            let arg_summary = arguments.as_object().unwrap().iter()
                .take(3)  // Show at most 3 parameters for preview
                .map(|(k, v)| {
                    let value_preview = match v {
                        Value::String(s) if s.len() > 25 => format!("\"{:.25}...\"", s),
                        Value::Array(a) => format!("[{} items]", a.len()),
                        Value::Object(o) => format!("{{...}} ({} fields)", o.len()),
                        _ => v.to_string(),
                    };
                    // Format each line with its own gray markup
                    format!("{}{}: {}{}", 
                        crate::constants::FORMAT_GRAY,
                        k, 
                        value_preview,
                        crate::constants::FORMAT_RESET
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
                
            let total_params = arguments.as_object().unwrap().len();
            let additional_params = if total_params > 3 {
                format!("{}+ {} more parameters{}", 
                    crate::constants::FORMAT_GRAY,
                    total_params - 3,
                    crate::constants::FORMAT_RESET
                )
            } else {
                String::new()
            };
                
            bprintln!("{}\n{}", 
                arg_summary,
                if !additional_params.is_empty() { additional_params } else { String::new() }
            );
        }
    }

    // Call the tool and get content
    match provider.get_tool_content(tool_name, arguments).await {
        Ok(contents) => {
            // Create a preview of the content
            let preview = if !contents.is_empty() {
                let mut preview_text = format!("Received {} content objects", contents.len());
                let mut items_previewed = 0;
                
                for content in contents.iter().take(3) {
                    if let crate::mcp::protocol::content::Content::Text(text) = content {
                        if items_previewed > 0 {
                            preview_text.push_str("\n---\n");
                        }
                        
                        // Preview first few lines of text
                        let lines: Vec<&str> = text.text.lines().take(5).collect();
                        if !lines.is_empty() {
                            preview_text.push_str(&format!("\n{}", lines.join("\n")));
                            
                            // Indicate if we truncated
                            let line_count = text.text.lines().count();
                            if line_count > 5 {
                                preview_text.push_str(&format!("\n[...truncated, {} more lines]", line_count - 5));
                            }
                            items_previewed += 1;
                        }
                    } else {
                        if items_previewed > 0 {
                            preview_text.push_str("\n---\n");
                        }
                        preview_text.push_str("[Resource content]");
                        items_previewed += 1;
                    }
                }
                
                // If there are more items than we previewed, indicate them
                if contents.len() > 3 {
                    preview_text.push_str(&format!("\n[+ {} additional content items not shown]", contents.len() - 3));
                }
                
                format!("{}{}{}", 
                    crate::constants::FORMAT_GRAY,
                    preview_text,
                    crate::constants::FORMAT_RESET
                )
            } else {
                format!("{}No content available{}", 
                    crate::constants::FORMAT_GRAY,
                    crate::constants::FORMAT_RESET
                )
            };

            if !silent_mode {
                // Bold completion message with format similar to read tool
                bprintln!(
                    tool: "mcp",
                    "{}🔌 Completed: {}.{} ({} content items){}",
                    crate::constants::FORMAT_BOLD,
                    server_name,
                    tool_name,
                    contents.len(),
                    crate::constants::FORMAT_RESET
                );

                // Create a preview for console output in read tool style
                let preview_content = contents.iter()
                    .take(1)  // Take only the first content item for preview
                    .filter_map(|content| {
                        if let crate::mcp::protocol::content::Content::Text(text) = content {
                            // Format each line separately with gray formatting
                            let preview_lines = text.text.lines()
                                .take(3)  // Take first 3 lines
                                .map(|line| format!("{}{}{}", 
                                    crate::constants::FORMAT_GRAY,
                                    line,
                                    crate::constants::FORMAT_RESET
                                ))
                                .collect::<Vec<String>>()
                                .join("\n");
                            
                            let total_lines = text.text.lines().count();
                            if total_lines > 3 {
                                // Show line count for additional lines with separate formatting
                                Some(format!(
                                    "{}\n{}+ {} more lines{}",
                                    preview_lines,
                                    crate::constants::FORMAT_GRAY,
                                    total_lines - 3,
                                    crate::constants::FORMAT_RESET
                                ))
                            } else {
                                Some(preview_lines)
                            }
                        } else {
                            // Non-text content
                            Some(format!(
                                "{}[Non-text content]{}",
                                crate::constants::FORMAT_GRAY,
                                crate::constants::FORMAT_RESET
                            ))
                        }
                    })
                    .collect::<Vec<String>>()
                    .join("\n");

                if !preview_content.is_empty() {
                    bprintln!("{}", preview_content);
                }
            }

            // Format the content for the agent in read-tool style
            let formatted_content: Vec<crate::llm::Content> = contents.into_iter()
                .enumerate()
                .map(|(i, c)| {
                    if let crate::mcp::protocol::content::Content::Text(text) = &c {
                        // Format text content like read tool does
                        let total_lines = text.text.lines().count();
                        let formatted_text = format!(
                            "Result: {}.{} (content item {}, {} lines)\n\n```\n{}\n```",
                            server_name,
                            tool_name,
                            i + 1,
                            total_lines,
                            text.text
                        );
                        crate::llm::Content::Text { text: formatted_text }
                    } else {
                        c.to_llm_content()
                    }
                })
                .collect();
                
            ToolResult {
                success: true,
                state_change: AgentStateChange::Continue,
                content: formatted_content,
            }
        }
        Err(err) => {
            // Format error message for the agent in a read-tool-like format
            let error_msg = format!(
                "Error: {}.{} - {}\n\nThe MCP tool execution failed. Please check the tool name and parameters.",
                server_name, tool_name, err
            );

            if !silent_mode {
                // Bold error message header similar to read tool's error formatting
                bprintln!(
                    "{}🔌 Error: {}.{} failed{}",
                    crate::constants::FORMAT_BOLD,
                    server_name,
                    tool_name,
                    crate::constants::FORMAT_RESET
                );
                
                // Error details in gray
                bprintln!("{}{}{}",
                    crate::constants::FORMAT_GRAY,
                    err,
                    crate::constants::FORMAT_RESET
                );
            }

            ToolResult::error(error_msg)
        }
    }
}