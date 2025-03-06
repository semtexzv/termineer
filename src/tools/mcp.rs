//! MCP tool for interacting with Model Context Protocol servers

use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    ToolResult::error(
        "MCP servers are configured in .termineer/config.json and initialized at startup. User commands for MCP are not needed as the system handles MCP functionality automatically.".to_string()
    )
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

    // Call tool
    if !silent_mode {
        bprintln !(tool: "mcp",
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
                bprintln !(tool: "mcp",
                    "Tool call successful: {} on {}",
                    tool_id,
                    server_name
                );
                bprintln!("{}", output);
            }

            // Return success with the MCP content objects
            ToolResult::success_from_mcp(contents)
        }
        Err(err) => {
            let error_msg = format!("Tool execution failed: {}", err);

            if !silent_mode {
                bprintln !(error:"{}", error_msg);
            }

            ToolResult::error(error_msg)
        }
    }
}
