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
/// @param args The tool arguments (first argument is the tool name)
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
        // If no tool name is provided, list available tools on this server
        return list_server_tools(server_name, silent_mode).await;
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

/// List tools available on a specific MCP server
///
/// This is called when a server name is used as a tool but no tool name is specified.
async fn list_server_tools(server_name: &str, silent_mode: bool) -> ToolResult {
    // Get provider using the MCP API
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

    // Refresh tools to ensure we have the latest
    match provider.refresh_tools().await {
        Ok(()) => {}
        Err(err) => {
            if !silent_mode {
                bprintln!(error: "Failed to refresh tools: {}", err);
            }

            return ToolResult::error(format!("Failed to refresh tools: {}", err));
        }
    }

    // List tools
    let tools = provider.list_tools().await;

    // Use consistent tool formatting with bold invocation and gray content
    if !silent_mode {
        // Bold invocation message
        bprintln!(
            "{}🔌 {}:{} Listing available tools",
            crate::constants::FORMAT_BOLD,
            server_name,
            crate::constants::FORMAT_RESET
        );
        
        // Gray content
        if !tools.is_empty() {
            let preview_count = std::cmp::min(5, tools.len());
            
            // Format tool info
            let tool_lines = tools.iter().take(preview_count)
                .map(|t| format!("- {}: {}", t.name, t.description))
                .collect::<Vec<_>>();
                
            // Show total count info
            let count_info = format!("Found {} tools total", tools.len());
                
            // Add truncation notice if needed
            let truncation_notice = if tools.len() > preview_count {
                format!("[+ {} more tools not shown]", tools.len() - preview_count)
            } else {
                String::new()
            };
            
            // Build the preview with everything in gray
            let preview = format!("{}{}{}{}{}{}",
                crate::constants::FORMAT_GRAY,
                count_info,
                if !tool_lines.is_empty() { "\n" } else { "" },
                tool_lines.join("\n"),
                if !truncation_notice.is_empty() { "\n" } else { "" },
                truncation_notice
            );
            
            bprintln!("{}{}", preview, crate::constants::FORMAT_RESET);
        } else {
            bprintln!("{}No tools available{}", 
                crate::constants::FORMAT_GRAY, 
                crate::constants::FORMAT_RESET
            );
        }
    }
    
    // Build output for agent
    let mut output = format!("Available tools on MCP server '{}':\n\n", server_name);

    if tools.is_empty() {
        output.push_str("No tools available.");
    } else {
        output.push_str("To use an MCP tool, invoke it directly with: `<server_name> <tool_name>`\n\n");
        
        for tool in &tools {
            output.push_str(&format!("### {}\n", tool.name));
            output.push_str(&format!("{}\n\n", tool.description));
            
            // Extract parameters from JSON Schema
            if let Some(properties) = tool.input_schema
                .as_object()
                .and_then(|schema| schema.get("properties"))
                .and_then(|props| props.as_object()) 
            {
                if !properties.is_empty() {
                    output.push_str("**Parameters:**\n");
                    
                    for (name, schema) in properties {
                        // Extract type information
                        let type_str = if let Some(type_val) = schema.get("type") {
                            type_val.as_str().unwrap_or("any").to_string()
                        } else {
                            "any".to_string()
                        };
                        
                        // Extract description
                        let desc = if let Some(desc_val) = schema.get("description") {
                            if let Some(desc_str) = desc_val.as_str() {
                                if !desc_str.is_empty() {
                                    format!("({})", desc_str)
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };
                        
                        output.push_str(&format!("- `{}`: {} {}\n", name, type_str, desc));
                    }
                    
                    output.push_str("\n");
                }
            }
        }
        
        // Add usage example
        output.push_str("**Example usage:**\n");
        if !tools.is_empty() {
            let example_tool = &tools[0];
            output.push_str(&format!("```\n{} {}\n", server_name, example_tool.name));
            output.push_str("{\n");
            
            // Extract parameters from JSON Schema for example
            let has_params = if let Some(properties) = example_tool.input_schema
                .as_object()
                .and_then(|schema| schema.get("properties"))
                .and_then(|props| props.as_object()) 
            {
                if !properties.is_empty() {
                    // Take up to 2 parameters for the example
                    let param_names: Vec<&String> = properties.keys().take(2).collect();
                    for name in param_names {
                        output.push_str(&format!("  \"{}\": \"value\",\n", name));
                    }
                    
                    if properties.len() > 2 {
                        output.push_str("  // other parameters as needed\n");
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            };
            
            if !has_params {
                output.push_str("  // No parameters required\n");
            }
            
            output.push_str("}\n```\n");
        }
    }

    ToolResult::success(output)
}