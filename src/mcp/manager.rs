//! MCP Manager for handling Model Context Protocol servers and tools

use crate::mcp::protocol::tools::Tool;
use crate::mcp::tool_provider::McpToolProvider;
use anyhow::format_err;
use lazy_static::lazy_static;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Enhanced tool information with examples for documentation
#[derive(Clone, Debug)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub example_request: String,
}

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
    #[allow(dead_code)]
    pub fn get_provider_or_error(&self, name: &str) -> anyhow::Result<Arc<McpToolProvider>> {
        match self.get_provider(name) {
            Some(provider) => Ok(provider),
            None => Err(format_err!("MCP provider not found: {}", name)),
        }
    }

    /// Check if there are any providers registered
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Get detailed tool information for all providers with examples
    pub fn get_tools_info(&self) -> HashMap<String, Vec<ToolInfo>> {
        let mut tools_map = HashMap::new();

        // Get tool information from each provider
        for (server_name, provider) in self.providers.iter() {
            // We need to use runtime for this async call
            let tools = provider.list_tools();

            if !tools.is_empty() {
                let mut tool_info = Vec::new();

                for tool in tools {
                    let description = if tool.description.is_empty() {
                        "No description".to_string()
                    } else {
                        tool.description.clone()
                    };

                    // Generate example request based on parameters
                    let mut example_request = HashMap::new();

                    // Check if the tool has a schema with properties
                    let input_schema = &tool.input_schema;
                    if let Some(properties) = &input_schema.properties {
                        if !properties.is_empty() {
                            for (param_name, schema) in properties.iter() {
                                // Get example value for this parameter
                                let example_value = get_example_value(schema, param_name);

                                example_request.insert(param_name.clone(), example_value);
                            }
                        }
                    }

                    tool_info.push(ToolInfo {
                        name: tool.name.clone(),
                        description,
                        example_request: serde_json::to_string_pretty(&example_request).unwrap()
                            + "\n",
                    });
                }

                tools_map.insert(server_name.clone(), tool_info);
            }
        }

        tools_map
    }

    /// Get all tools for a specific provider
    #[allow(dead_code)]
    pub fn get_tools_for_provider(&self, provider_name: &str) -> Vec<Tool> {
        if let Some(provider) = self.get_provider(provider_name) {
            provider.list_tools()
        } else {
            Vec::new()
        }
    }

    /// List all tools across all providers
    #[allow(dead_code)]
    pub async fn list_all_tools(&self) -> HashMap<String, Vec<Tool>> {
        let mut result = HashMap::new();

        for (name, provider) in &self.providers {
            let tools = provider.list_tools();
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
/// Returns true if registration was successful, false if it failed or was rejected
pub fn register_provider(name: &str, provider: Arc<McpToolProvider>) -> bool {
    // Check if the name collides with a built-in tool name
    if is_built_in_tool_name(name) {
        // Log the rejection
        bprintln!(error:
            "MCP server name '{}' rejected: Name collides with a built-in tool",
            name
        );
        return false;
    }

    // Register the provider
    if let Ok(mut manager) = MCP_MANAGER.lock() {
        manager.register(name, provider);
        true
    } else {
        false
    }
}

/// Check if a name conflicts with a built-in tool name
fn is_built_in_tool_name(name: &str) -> bool {
    // Convert name to lowercase for case-insensitive comparison
    let name_lower = name.to_lowercase();

    // Check against standard tools
    let standard_tools = crate::prompts::ALL_TOOLS;
    for tool in standard_tools {
        if tool.to_lowercase() == name_lower {
            return true;
        }
    }

    // Check against plus tools
    let plus_tools = crate::prompts::PLUS_TOOLS;
    for tool in plus_tools {
        if tool.to_lowercase() == name_lower {
            return true;
        }
    }

    // No collision found
    false
}

/// Generate a representative example value for a schema property type
/// following JSON Schema conventions used in MCP
fn get_example_value(
    property_schema: &crate::mcp::protocol::schema::PropertySchema,
    param_name: &str,
) -> serde_json::Value {
    use crate::mcp::protocol::schema::SchemaType;

    // Format values as JSON for tool examples
    match &property_schema.property_type {
        Some(SchemaType::String) => {
            // Use enum values if available
            if let Some(enum_values) = &property_schema.enum_values {
                if !enum_values.is_empty() {
                    return json!("example"); // Simple string example
                }
            }

            // Use format hints if available
            if let Some(format) = &property_schema.format {
                match format.as_str() {
                    "date" => return json!("2024-05-01"),
                    "date-time" => return json!("2024-05-01T12:00:00Z"),
                    "email" => return json!("user@example.com"),
                    "uri" => return json!("https://example.com"),
                    "uuid" => return json!("123e4567-e89b-12d3-a456-426614174000"),
                    _ => {}
                }
            }

            json!(format!("example_{}", param_name))
        }
        Some(SchemaType::Integer) => json!(42),
        Some(SchemaType::Number) => json!(42.3),
        Some(SchemaType::Boolean) => json!(true),
        Some(SchemaType::Object) => json!({"prop": false}),
        Some(SchemaType::Array) => json!([]),
        Some(SchemaType::Null) => json!(null),
        _ => json!(null),
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
/// - Map of server names to tool information (ToolInfo objects)
pub fn get_mcp_tools_for_prompt() -> (Vec<String>, HashMap<String, Vec<ToolInfo>>) {
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
        // Add detailed tools information
        let mut tools_array = Vec::new();

        for (server_name, tools) in tools_info {
            for tool_info in tools {
                let mut tool_obj = serde_json::Map::new();
                tool_obj.insert("server".to_string(), serde_json::json!(server_name));
                tool_obj.insert("name".to_string(), serde_json::json!(tool_info.name));
                tool_obj.insert(
                    "description".to_string(),
                    serde_json::json!(tool_info.description),
                );
                tool_obj.insert(
                    "example_request".to_string(),
                    serde_json::json!(tool_info.example_request),
                );
                tools_array.push(serde_json::json!(tool_obj));
            }
        }

        obj.insert("mcp_tools".to_string(), serde_json::json!(tools_array));
    }
}
