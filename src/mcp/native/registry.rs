//! Registry for MCP tools exposed as native tools

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::mcp::protocol::Tool;
use crate::mcp::tool_provider::McpToolProvider;
use crate::tools::ToolResult;

/// Represents an MCP tool registered as a native tool
#[derive(Debug, Clone)]
pub struct RegisteredMcpTool {
    /// The server name this tool belongs to
    pub server_name: String,
    /// The original tool name in the MCP server
    pub original_tool_name: String,
    /// Reference to the tool provider for execution
    pub provider: Arc<McpToolProvider>,
    /// Tool definition from the MCP server
    pub tool_info: Tool,
}

lazy_static! {
    /// Global registry mapping native tool names to their MCP implementation
    pub static ref MCP_NATIVE_TOOLS: Mutex<HashMap<String, RegisteredMcpTool>> = Mutex::new(HashMap::new());
}

/// Register an MCP tool as a native tool
pub async fn register_native_tool(
    native_name: String,
    server_name: String,
    original_tool_name: String,
    provider: Arc<McpToolProvider>,
    tool_info: Tool,
    silent_mode: bool,
) -> Result<(), String> {
    let mut registry = MCP_NATIVE_TOOLS.lock().await;
    
    // Check if the tool name already exists
    if registry.contains_key(&native_name) {
        if !silent_mode {
            bprintln!(warn: "Native tool name '{}' already registered, skipping", native_name);
        }
        return Err(format!("Tool name '{}' already registered", native_name));
    }
    
    // Register the tool
    registry.insert(
        native_name.clone(),
        RegisteredMcpTool {
            server_name,
            original_tool_name,
            provider,
            tool_info,
        },
    );
    
    if !silent_mode {
        bprintln!(info: "Registered MCP tool '{}' as native tool '{}'", original_tool_name, native_name);
    }
    
    Ok(())
}

/// Execute an MCP tool registered as a native tool
pub async fn execute_native_mcp_tool(
    tool_name: &str,
    args: &str,
    body: &str,
    silent_mode: bool,
) -> Option<ToolResult> {
    // Get the registered tool
    let registered_tool = {
        let registry = MCP_NATIVE_TOOLS.lock().await;
        registry.get(tool_name).cloned()
    };
    
    // If tool not found in registry, return None to let the regular tool system handle it
    let registered_tool = match registered_tool {
        Some(tool) => tool,
        None => return None,
    };
    
    // Parse tool arguments from the combination of args and body
    let arguments = if args.trim().is_empty() && body.trim().is_empty() {
        // No arguments provided, use empty object
        serde_json::json!({})
    } else if !args.trim().is_empty() {
        // Try to parse args as JSON if provided
        match serde_json::from_str::<serde_json::Value>(args) {
            Ok(value) => value,
            Err(_) => {
                // If args is not valid JSON, try combining with body
                let combined = if body.trim().is_empty() {
                    // If body is empty, treat args as a string value
                    serde_json::json!(args.trim())
                } else {
                    // If body is not empty, build a custom object
                    serde_json::json!({
                        "args": args.trim(),
                        "body": body.trim()
                    })
                };
                combined
            }
        }
    } else {
        // Use body as JSON
        match serde_json::from_str::<serde_json::Value>(body) {
            Ok(value) => value,
            Err(_) => {
                // If body is not valid JSON, use it as a string
                serde_json::json!(body.trim())
            }
        }
    };
    
    // Log the call with better formatting to match other tools
    if !silent_mode {
        // Bold invocation message
        bprintln!(tool: tool_name,
            "{}🔌 MCP:{} Executing native tool '{}' via server '{}'",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            registered_tool.original_tool_name,
            registered_tool.server_name
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
    
    // Execute the tool using the provider
    let provider = registered_tool.provider;
    let original_tool_name = registered_tool.original_tool_name;
    
    match provider.get_tool_content(&original_tool_name, arguments).await {
        Ok(contents) => {
            // Log success
            if !silent_mode {
                bprintln!(tool: tool_name,
                    "{}✅ MCP completed:{} '{}'",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET,
                    original_tool_name
                );
            }
            
            // Return success with the MCP content objects
            Some(ToolResult::success_from_mcp(contents))
        },
        Err(err) => {
            // Format error message
            let error_msg = format!(
                "MCP tool execution failed: '{}' - {}", 
                original_tool_name, 
                err
            );
            
            if !silent_mode {
                bprintln!(error: "{}", error_msg);
            }
            
            Some(ToolResult::error(error_msg))
        }
    }
}

/// List all registered native MCP tools
pub async fn list_registered_native_tools() -> Vec<String> {
    let registry = MCP_NATIVE_TOOLS.lock().await;
    registry.keys().cloned().collect()
}

/// Get information about a native MCP tool
pub async fn get_native_tool_info(name: &str) -> Option<RegisteredMcpTool> {
    let registry = MCP_NATIVE_TOOLS.lock().await;
    registry.get(name).cloned()
}