//! MCP tool for interacting with Model Context Protocol servers

use anyhow::format_err;
use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::mcp::protocol::content;
use crate::mcp::protocol::content::TextContent;
use crate::mcp::tool_provider::McpToolProvider;
use crate::tools::{AgentStateChange, ToolResult};

/// Singleton manager for all MCP tool providers
pub struct McpManager {
    /// Map of provider names to provider instances
    providers: HashMap<String, Arc<McpToolProvider>>,
}

impl McpManager {
    /// Create a new empty MCP manager
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
    
    /// Register a provider with the manager
    pub fn register(&mut self, name: &str, provider: Arc<McpToolProvider>) {
        self.providers.insert(name.to_string(), provider);
    }
    
    /// Check if a provider exists
    pub fn has_provider(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }
    
    /// Get a provider by name
    pub fn get_provider(&self, name: &str) -> Option<Arc<McpToolProvider>> {
        self.providers.get(name).cloned()
    }
    
    /// Get all provider names
    pub fn get_provider_names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }
    
    /// Get a provider by name and return error if not found
    pub fn get_provider_or_error(&self, name: &str) -> anyhow::Result<Arc<McpToolProvider>> {
        match self.get_provider(name) {
            Some(provider) => Ok(provider),
            None => Err(format_err!("MCP provider not found: {}", name))
        }
    }
    
    /// Check if there are any providers registered
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
    
    /// Get detailed tool information for all providers
    pub fn get_tools_info(&self) -> HashMap<String, Vec<(String, String)>> {
        let mut tools_map = HashMap::new();
        
        // Create a runtime for calling async methods on providers
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        
        // Get tool information from each provider
        for (server_name, provider) in self.providers.iter() {
            // We need to use runtime for this async call
            let tools = runtime.block_on(provider.list_tools());
            
            if !tools.is_empty() {
                let mut tool_info = Vec::new();
                
                for tool in tools {
                    let description = if tool.description.is_empty() {
                        "No description".to_string()
                    } else {
                        tool.description.clone()
                    };
                    
                    tool_info.push((tool.name.clone(), description));
                }
                
                tools_map.insert(server_name.clone(), tool_info);
            }
        }
        
        tools_map
    }
    
    /// Get all tools for a specific provider
    pub fn get_tools_for_provider(&self, provider_name: &str) -> Vec<crate::mcp::protocol::tools::Tool> {
        if let Some(provider) = self.get_provider(provider_name) {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            
            runtime.block_on(provider.list_tools())
        } else {
            Vec::new()
        }
    }
    
    /// List all tools across all providers
    pub async fn list_all_tools(&self) -> HashMap<String, Vec<crate::mcp::protocol::tools::Tool>> {
        let mut result = HashMap::new();
        
        for (name, provider) in &self.providers {
            let tools = provider.list_tools().await;
            if !tools.is_empty() {
                result.insert(name.clone(), tools);
            }
        }
        
        result
    }
}

// Private module-level singleton
lazy_static! {
    /// Global MCP manager instance
    static ref MCP_MANAGER: Mutex<McpManager> = Mutex::new(McpManager::new());
}

// Public API - all interaction with MCP providers happens through these functions

/// Register a provider with the MCP manager
pub fn register_provider(name: &str, provider: Arc<McpToolProvider>) -> bool {
    if let Ok(mut manager) = MCP_MANAGER.lock() {
        manager.register(name, provider);
        true
    } else {
        false
    }
}

/// Check if a provider with the given name exists
pub fn has_provider(name: &str) -> bool {
    if let Ok(manager) = MCP_MANAGER.lock() {
        manager.has_provider(name)
    } else {
        false
    }
}

/// Get a provider by name (returns a cloned Arc)
pub fn get_provider(name: &str) -> Option<Arc<McpToolProvider>> {
    if let Ok(manager) = MCP_MANAGER.lock() {
        manager.get_provider(name)
    } else {
        None
    }
}

/// Get the list of all provider names
pub fn get_provider_names() -> Vec<String> {
    if let Ok(manager) = MCP_MANAGER.lock() {
        manager.get_provider_names()
    } else {
        Vec::new()
    }
}

/// Check if there are any MCP providers registered
pub fn has_providers() -> bool {
    if let Ok(manager) = MCP_MANAGER.lock() {
        !manager.is_empty()
    } else {
        false
    }
}

/// Get the current MCP tools information in a format suitable for prompt generation
/// 
/// Returns a tuple of:
/// - List of server names
/// - Map of server names to tool information (name, description)
pub fn get_mcp_tools_for_prompt() -> (Vec<String>, HashMap<String, Vec<(String, String)>>) {
    if let Ok(manager) = MCP_MANAGER.lock() {
        let names = manager.get_provider_names();
        let tools_info = manager.get_tools_info();
        (names, tools_info)
    } else {
        // In case of a poisoned mutex, return empty data
        (Vec::new(), HashMap::new())
    }
}

/// Helper function to update MCP tools in prompts
///
/// This function will add all MCP tools information to the prompt template data.
pub fn add_mcp_tools_to_prompt(template_data: &mut serde_json::Value) {
    // Get detailed MCP tools information
    let (server_names, tools_info) = get_mcp_tools_for_prompt();
    
    // Skip if no MCP servers are configured
    if server_names.is_empty() {
        return;
    }
    
    // Add server names array to template data
    if let Some(obj) = template_data.as_object_mut() {
        // Add MCP server names
        obj.insert("mcp_servers".to_string(), serde_json::json!(server_names));
        
        // Add detailed tools information
        let mut mcp_tools_json = serde_json::Map::new();
        
        for (server_name, tools) in tools_info {
            let mut tools_array = Vec::new();
            
            for (tool_name, description) in tools {
                let mut tool_obj = serde_json::Map::new();
                tool_obj.insert("name".to_string(), serde_json::json!(tool_name));
                tool_obj.insert("description".to_string(), serde_json::json!(description));
                tools_array.push(serde_json::json!(tool_obj));
            }
            
            mcp_tools_json.insert(server_name, serde_json::json!(tools_array));
        }
        
        obj.insert("mcp_tools".to_string(), serde_json::json!(mcp_tools_json));
    }
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
    // Get the list of currently configured servers using our McpManager API
    let server_names = get_provider_names();
    
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
    // Get provider using the McpManager API
    let provider = match get_provider(server_name) {
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
    // Get provider using the McpManager API
    let provider = match get_provider(server_name) {
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
