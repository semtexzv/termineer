//! MCP tool for interacting with Model Context Protocol servers

use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::mcp::protocol::content;
use crate::mcp::protocol::content::TextContent;
use crate::mcp::tool_provider::McpToolProvider;
use crate::tools::{AgentStateChange, ToolResult};

lazy_static! {
    /// Global map of MCP tool providers, indexed by server ID (URL, process name, or friendly name)
    pub(crate) static ref MCP_PROVIDERS: Mutex<HashMap<String, Arc<McpToolProvider>>> = Mutex::new(HashMap::new());
}

/// Execute the MCP tool with the given arguments and body
pub async fn execute_mcp_tool(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse the arguments to determine the subcommand
    let args_parts: Vec<&str> = args.trim().splitn(3, ' ').collect();
    if args_parts.is_empty() {
        return ToolResult::error(
            "MCP servers are configured in .termineer/config.json and initialized at startup."
                .to_string(),
        );
    }

    let subcommand = args_parts[0].to_lowercase();

    // If this is a call subcommand with proper args, process it (for agents)
    if subcommand == "call" && args_parts.len() >= 3 {
        let server_name = args_parts[1].trim();
        let tool_id = args_parts[2].trim();

        // Make sure we have a body for JSON
        if body.trim().is_empty() {
            return ToolResult::error(
                "MCP call requires a JSON body for the tool parameters.".to_string(),
            );
        }

        // Try to call the tool
        return execute_call_by_name(server_name, tool_id, body, silent_mode).await;
    }

    // For the list command, allow it for agents
    if subcommand == "list" && args_parts.len() >= 2 {
        let server_name = args_parts[1].trim();
        return execute_list_by_name(server_name, silent_mode).await;
    }

    // For all other cases (including user attempts), return an informational message
    if !silent_mode {
        bprintln!(
            "MCP tools are configured via .termineer/config.json and used automatically by the system."
        );
    }

    // Return a message explaining that MCP is handled automatically
    // Get the list of currently configured servers
    let providers = MCP_PROVIDERS.lock().await;
    let server_names: Vec<String> = providers.keys().cloned().collect();
    
    // Build a helpful message
    let mut message = "MCP (Model Context Protocol) servers provide additional capabilities to AI assistants.\n\n".to_string();
    
    if !server_names.is_empty() {
        message.push_str("Currently configured MCP servers:\n");
        for name in &server_names {
            message.push_str(&format!("- {}\n", name));
        }
        message.push_str("\nTo list tools: mcp list SERVER_NAME\n");
        message.push_str("To call a tool: mcp call SERVER_NAME TOOL_NAME with JSON parameters\n");
    } else {
        message.push_str("No MCP servers are currently configured.\n");
        message.push_str("Configure servers in .termineer/config.json\n");
    }
    
    ToolResult::success(message)
}

/// List tools from an MCP server by server name
pub async fn execute_list_by_name(server_name: &str, silent_mode: bool) -> ToolResult {
    // Get provider by exact name (since we're now using friendly names)
    let provider = {
        let providers = MCP_PROVIDERS.lock().await;
        providers.get(server_name).cloned()
    };

    // Check if we found a provider
    let provider = match provider {
        Some(provider) => provider,
        None => {
            if !silent_mode {
                bprintln !(error:"MCP server not found: {}", server_name);
            }

            return ToolResult::error(format!(
                "MCP server not found: {}. Available servers can be seen at startup.",
                server_name
            ));
        }
    };

    // Refresh tools
    match provider.refresh_tools().await {
        Ok(()) => {}
        Err(err) => {
            if !silent_mode {
                bprintln !(error:
                    "Failed to refresh tools: {}",
                    err
                );
            }

            return ToolResult::error(format!("Failed to refresh tools: {}", err));
        }
    }

    // List tools
    let tools = provider.list_tools().await;

    // Use consistent tool formatting with bold invocation and gray content
    if !silent_mode {
        // Bold invocation message
        bprintln!(tool: "mcp",
            "{}🔌 MCP:{} Listing tools on server {}",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            server_name
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
    let mut output = format!("Tools available from MCP server '{}':\n\n", server_name);

    if tools.is_empty() {
        output.push_str("No tools available.");
    } else {
        for tool in &tools {
            output.push_str(&format!("- Name: {}\n", tool.name));
            output.push_str(&format!("  Description: {}\n", tool.description));

            // Show input schema if it's a simple object
            if let Value::Object(obj) = &tool.input_schema {
                if let Some(Value::Object(props)) = obj.get("properties") {
                    output.push_str("  Parameters:\n");
                    for (name, schema) in props {
                        let type_str = schema.get("type").and_then(|v| v.as_str()).unwrap_or("any");
                        let desc = schema
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(|d| format!("({})", d))
                            .unwrap_or(String::new());
                        output.push_str(&format!("    - {}: {} {}\n", name, type_str, desc));
                    }
                }
            }

            output.push('\n');
        }
    }

    ToolResult {
        success: true,
        state_change: AgentStateChange::Continue,
        content: vec![crate::llm::Content::Text { text: output }],
    }
}

/// Call a tool on an MCP server by server name
pub async fn execute_call_by_name(
    server_name: &str,
    tool_id: &str,
    body: &str,
    silent_mode: bool,
) -> ToolResult {
    // Get provider by exact name (since we're now using friendly names)
    let provider = {
        let providers = MCP_PROVIDERS.lock().await;
        providers.get(server_name).cloned()
    };

    // Check if we found a provider
    let provider = match provider {
        Some(provider) => provider,
        None => {
            if !silent_mode {
                bprintln !(error:"MCP server not found: {}", server_name);
            }

            return ToolResult::error(format!(
                "MCP server not found: {}. Available servers can be seen at startup.",
                server_name
            ));
        }
    };

    // Parse tool arguments from body
    let arguments: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(err) => {
            if !silent_mode {
                bprintln !(error:"Failed to parse tool arguments: {}", err);
            }

            return ToolResult::error(format!(
                "Failed to parse tool arguments: {}. Arguments must be valid JSON.",
                err
            ));
        }
    };

    // Get tool info for better messaging (for logging purposes)
    let _tool_info = provider
        .get_tool(tool_id)
        .await
        .map(|t| format!("{} ({})", t.name, t.description))
        .unwrap_or_else(|| tool_id.to_string());

    // We don't need this with our simplified logging approach

    // Log the call with better formatting to match other tools
    if !silent_mode {
        // Bold invocation message
        bprintln !(tool: "mcp",
            "{}🔌 MCP:{} Calling {} on server {}",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            tool_id,
            server_name
        );
        
        // Show argument summary in gray if available
        if arguments.is_object() && !arguments.as_object().unwrap().is_empty() {
            let arg_keys = arguments.as_object().unwrap().keys()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ");
                
            bprintln!("{}Arguments: {}{}", 
                crate::constants::FORMAT_GRAY,
                arg_keys,
                crate::constants::FORMAT_RESET
            );
        }
    }

    // Execute the tool and get content objects
    match provider.get_tool_content(tool_id, arguments).await {
        Ok(contents) => {
            // Get preview of the actual content
            let preview = if !contents.is_empty() {
                let mut preview_text = format!("Received {} content item(s):\n", contents.len());
                
                // Try to preview up to 3 content items
                let mut items_previewed = 0;
                
                for content_item in contents.iter().take(3) {
                    match content_item {
                        content::Content::Text(TextContent { text, .. }) => {
                            // Add separator between items if needed
                            if items_previewed > 0 {
                                preview_text.push_str("\n---\n");
                            }
                            
                            // Get up to 10 lines for preview
                            let lines: Vec<&str> = text.lines().take(10).collect();
                            if !lines.is_empty() {
                                preview_text.push_str(&lines.join("\n"));
                                
                                // Indicate if content was truncated
                                let line_count = text.lines().count();
                                if line_count > 10 {
                                    preview_text.push_str(&format!("\n[...truncated, {} more lines]", line_count - 10));
                                }
                                
                                items_previewed += 1;
                            }
                        },
                        content::Content::Image(_) => {
                            // Just note image content without trying to display
                            if items_previewed > 0 {
                                preview_text.push_str("\n---\n");
                            }
                            preview_text.push_str("[Image content]");
                            items_previewed += 1;
                        },
                        content::Content::Resource(_) => {
                            // Just note resource content without trying to display
                            if items_previewed > 0 {
                                preview_text.push_str("\n---\n");
                            }
                            preview_text.push_str("[Resource content]");
                            items_previewed += 1;
                        }
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
                bprintln !(tool: "mcp",
                    "{}🔌 MCP completed:{} {} on {}",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET,
                    tool_id,
                    server_name
                );

                // Show preview in gray (preview already has gray formatting)
                if !preview.is_empty() {
                    bprintln!("{}", preview);
                }
            }

            // No need to create a variable for output that's not used

            // Return success with the MCP content objects
            ToolResult::success_from_mcp(contents)
        }
        Err(err) => {
            // Format error message for the agent
            let error_msg = format!("MCP call failed: {} on {} - {}", tool_id, server_name, err);

            if !silent_mode {
                // Bold error message header
                bprintln !(tool: "mcp",
                    "{}🔌 MCP error:{} {} on {}",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET,
                    tool_id,
                    server_name
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
