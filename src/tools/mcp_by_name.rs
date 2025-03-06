//! MCP helper functions for accessing servers by name

use crate::tools::ToolResult;
use serde_json::Value;

// Import the MCP providers map and content conversion traits
use crate::tools::mcp::MCP_PROVIDERS;
use crate::mcp::protocol::McpContent;
use crate::tools::ToolExecutor;

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
                crate::berror_println!("MCP server not found: {}", server_name);
            }
            
            return ToolResult::error(format!(
                "MCP server not found: {}. Available servers can be seen at startup.", 
                server_name
            ));
        }
    };
    
    // Refresh tools
    match provider.refresh_tools().await {
        Ok(()) => {},
        Err(err) => {
            if !silent_mode {
                crate::berror_println!(
                    "Failed to refresh tools: {}", 
                    err
                );
            }
            
            return ToolResult::error(format!(
                "Failed to refresh tools: {}", 
                err
            ));
        }
    }
    
    // List tools
    let tools = provider.list_tools().await;
    let tool_count = tools.len();
    
    // Format tool list
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
                        let desc = schema.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        output.push_str(&format!("    - {}: {} ({})\n", name, type_str, desc));
                    }
                }
            }
            
            output.push('\n');
        }
    }
    
    if !silent_mode {
        crate::btool_println!(
            "mcp",
            "Listed {} tools from MCP server: {}",
            tool_count,
            server_name
        );
        crate::bprintln!("{}", output);
    }
    
    ToolResult::success(output)
}

/// Call a tool on an MCP server by server name
pub async fn execute_call_by_name(server_name: &str, tool_id: &str, body: &str, silent_mode: bool) -> ToolResult {
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
                crate::berror_println!("MCP server not found: {}", server_name);
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
                crate::berror_println!("Failed to parse tool arguments: {}", err);
            }
            
            return ToolResult::error(format!(
                "Failed to parse tool arguments: {}. Arguments must be valid JSON.", 
                err
            ));
        }
    };
    
    // Get tool info for better messaging
    // Since the Tool struct doesn't have an id field, we'll use the tool_id 
    // parameter directly and try to get more information if available
    let tool_info = provider.get_tool(tool_id).await.map(|t| format!(
        "{} ({})", 
        t.name, 
        t.description
    )).unwrap_or_else(|| tool_id.to_string());
    
    // Call tool
    if !silent_mode {
        crate::btool_println!(
            "mcp",
            "Calling tool {} on MCP server: {}", 
            tool_id,
            server_name
        );
    }
    
    // Execute the tool and get content objects
    match provider.get_tool_content(tool_id, arguments).await {
        Ok(contents) => {
            // Format a summary for the user
            let output = format!(
                "Tool call successful: {} on {}\nReturned {} content items that will be added to the conversation.",
                tool_id,
                server_name,
                contents.len()
            );
            
            if !silent_mode {
                crate::btool_println!(
                    "mcp",
                    "Tool call successful: {} on {}",
                    tool_id,
                    server_name
                );
                crate::bprintln!("{}", output);
            }
            
            // Return success with the MCP content objects
            ToolResult::success_with_mcp_content(contents)
        },
        Err(err) => {
            let error_msg = format!("Tool execution failed: {}", err);
            
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
            }
            
            ToolResult::error(error_msg)
        }
    }
}