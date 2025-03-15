//! MCP Manager for handling Model Context Protocol servers and tools

use anyhow::format_err;
use lazy_static::lazy_static;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::mcp::protocol::tools::Tool;
use crate::mcp::tool_provider::McpToolProvider;

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
    pub fn get_tools_for_provider(&self, provider_name: &str) -> Vec<Tool> {
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
    pub async fn list_all_tools(&self) -> HashMap<String, Vec<Tool>> {
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