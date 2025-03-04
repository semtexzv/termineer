//! MCP tool for interacting with Model Context Protocol servers

use std::collections::HashMap;
use std::sync::Arc;
use lazy_static::lazy_static;
use tokio::sync::Mutex;

use crate::tools::ToolResult;
use crate::mcp::tool_provider::McpToolProvider;
use crate::mcp::error::McpError;

lazy_static! {
    /// Global map of MCP tool providers, indexed by server URL
    static ref MCP_PROVIDERS: Mutex<HashMap<String, Arc<McpToolProvider>>> = Mutex::new(HashMap::new());
}

/// Execute the MCP tool with the given arguments and body
pub async fn execute_mcp_tool(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse the arguments to determine the subcommand
    let args_parts: Vec<&str> = args.trim().splitn(2, ' ').collect();
    if args_parts.is_empty() {
        return ToolResult::error(
            "MCP tool requires a subcommand (connect, list, call)".to_string()
        );
    }
    
    let subcommand = args_parts[0].to_lowercase();
    let subcommand_args = args_parts.get(1).map(|s| s.trim()).unwrap_or("");
    
    // Execute the appropriate subcommand
    match subcommand.as_str() {
        "connect" => execute_connect(subcommand_args, silent_mode).await,
        "list" => execute_list(subcommand_args, silent_mode).await,
        "call" => execute_call(subcommand_args, body, silent_mode).await,
        _ => ToolResult::error(format!(
            "Unknown MCP subcommand: {}. Available subcommands: connect, list, call", 
            subcommand
        )),
    }
}

/// Connect to an MCP server
async fn execute_connect(args: &str, silent_mode: bool) -> ToolResult {
    // Parse server URL
    let server_url = args.trim();
    if server_url.is_empty() {
        return ToolResult::error("Server URL is required".to_string());
    }
    
    // Check if already connected
    {
        let providers = MCP_PROVIDERS.lock().await;
        if providers.contains_key(server_url) {
            if !silent_mode {
                crate::btool_println!(
                    "mcp",
                    "Already connected to MCP server: {}", 
                    server_url
                );
            }
            return ToolResult::success(format!(
                "Already connected to MCP server: {}", 
                server_url
            ));
        }
    }
    
    // Connect to server
    if !silent_mode {
        crate::btool_println!(
            "mcp",
            "Connecting to MCP server: {}", 
            server_url
        );
    }
    
    // Create provider
    match McpToolProvider::new(server_url).await {
        Ok(provider) => {
            let provider: Arc<McpToolProvider> = Arc::new(provider);
            
            // Get tool count
            let tool_count = provider.list_tools().await.len();
            
            // Store provider
            {
                let mut providers = MCP_PROVIDERS.lock().await;
                providers.insert(server_url.to_string(), provider);
            }
            
            if !silent_mode {
                crate::btool_println!(
                    "mcp",
                    "Connected to MCP server: {}. Found {} tools.",
                    server_url,
                    tool_count
                );
            }
            
            ToolResult::success(format!(
                "Connected to MCP server: {}. Found {} tools.", 
                server_url, 
                tool_count
            ))
        },
        Err(err) => {
            if !silent_mode {
                crate::berror_println!(
                    "Failed to connect to MCP server: {}", 
                    err
                );
            }
            
            ToolResult::error(format!(
                "Failed to connect to MCP server: {}", 
                err
            ))
        }
    }
}

/// List tools from an MCP server
async fn execute_list(args: &str, silent_mode: bool) -> ToolResult {
    // Parse server URL
    let server_url = args.trim();
    if server_url.is_empty() {
        return ToolResult::error("Server URL is required".to_string());
    }
    
    // Get provider
    let provider = {
        let providers = MCP_PROVIDERS.lock().await;
        providers.get(server_url).cloned()
    };
    
    // Check if connected
    let provider = match provider {
        Some(provider) => provider,
        None => {
            return ToolResult::error(format!(
                "Not connected to MCP server: {}. Use mcp connect first.", 
                server_url
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
    let mut output = format!("Tools available from MCP server {}:\n\n", server_url);
    
    if tools.is_empty() {
        output.push_str("No tools available.");
    } else {
        for tool in &tools {
            output.push_str(&format!("- ID: {}\n", tool.id));
            output.push_str(&format!("  Name: {}\n", tool.name));
            output.push_str(&format!("  Description: {}\n", tool.description));
            
            // Show input schema if it's a simple object
            if let serde_json::Value::Object(obj) = &tool.input_schema {
                if let Some(serde_json::Value::Object(props)) = obj.get("properties") {
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
            server_url
        );
        crate::bprintln!("{}", output);
    }
    
    ToolResult::success(output)
}

/// Call a tool on an MCP server
async fn execute_call(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse server URL and tool ID
    let parts: Vec<&str> = args.splitn(2, ' ').collect();
    if parts.len() < 2 {
        return ToolResult::error(
            "Server URL and tool ID are required".to_string()
        );
    }
    
    let server_url = parts[0].trim();
    let tool_id = parts[1].trim();
    
    // Get provider
    let provider = {
        let providers = MCP_PROVIDERS.lock().await;
        providers.get(server_url).cloned()
    };
    
    // Check if connected
    let provider = match provider {
        Some(provider) => provider,
        None => {
            return ToolResult::error(format!(
                "Not connected to MCP server: {}. Use mcp connect first.", 
                server_url
            ));
        }
    };
    
    // Parse tool input from body
    let input: serde_json::Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(err) => {
            if !silent_mode {
                crate::berror_println!(
                    "Failed to parse tool input: {}", 
                    err
                );
            }
            
            return ToolResult::error(format!(
                "Failed to parse tool input: {}. Input must be valid JSON.", 
                err
            ));
        }
    };
    
    // Get tool info
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
            server_url
        );
    }
    
    match provider.execute_tool(tool_id, input).await {
        Ok(result) => {
            // Format result as pretty JSON
            let pretty_result = match serde_json::to_string_pretty(&result) {
                Ok(s) => s,
                Err(_) => format!("{:?}", result),
            };
            
            let output = format!(
                "Result from tool {} on MCP server {}:\n\n{}", 
                tool_info, 
                server_url, 
                pretty_result
            );
            
            if !silent_mode {
                crate::btool_println!(
                    "mcp",
                    "Tool call successful: {} on {}",
                    tool_id,
                    server_url
                );
                crate::bprintln!("{}", output);
            }
            
            ToolResult::success(output)
        },
        Err(err) => {
            let error_msg = match err {
                McpError::ToolNotFound(_) => format!("Tool not found: {}", tool_id),
                McpError::Timeout => "Tool execution timed out".to_string(),
                McpError::JsonRpcError(rpc_err) => format!(
                    "RPC error (code {}): {}", 
                    rpc_err.code, 
                    rpc_err.message
                ),
                _ => format!("Tool execution failed: {}", err),
            };
            
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
            }
            
            ToolResult::error(error_msg)
        }
    }
}