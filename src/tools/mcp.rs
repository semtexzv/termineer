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

    // Use consistent tool formatting with bold invocation and gray preview
    if !silent_mode {
        // Bold invocation message
        bprintln!(
            "{}🔌 {}.{}:{} Executing MCP tool",
            crate::constants::FORMAT_BOLD,
            server_name,
            tool_name,
            crate::constants::FORMAT_RESET
        );
        
        // Gray content - show argument summary
        if arguments.is_object() && !arguments.as_object().unwrap().is_empty() {
            let arg_keys = arguments.as_object().unwrap().keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
                
            bprintln!("{}Parameters: {}{}", 
                crate::constants::FORMAT_GRAY,
                arg_keys,
                crate::constants::FORMAT_RESET
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
                // Bold completion message
                bprintln!(
                    "{}🔌 {}.{} completed{}",
                    crate::constants::FORMAT_BOLD,
                    server_name,
                    tool_name,
                    crate::constants::FORMAT_RESET
                );

                // Show preview in gray (preview already has gray formatting)
                if !preview.is_empty() {
                    bprintln!("{}", preview);
                }
            }

            // Convert MCP content to LLM content and return
            let llm_content = contents.into_iter()
                .map(|c| c.to_llm_content())
                .collect();
                
            ToolResult {
                success: true,
                state_change: AgentStateChange::Continue,
                content: llm_content,
            }
        }
        Err(err) => {
            // Format error message for the agent
            let error_msg = format!("MCP tool failed: {}.{} - {}", server_name, tool_name, err);

            if !silent_mode {
                // Bold error message header
                bprintln!(
                    "{}🔌 {}.{} error:{}",
                    crate::constants::FORMAT_BOLD,
                    server_name,
                    tool_name,
                    crate::constants::FORMAT_RESET
                );
                
                // Error details in gray
                bprintln!("{}Error: {}{}", 
                    crate::constants::FORMAT_GRAY,
                    err,
                    crate::constants::FORMAT_RESET
                );
            }

            ToolResult::error(error_msg)
        }
    }
}