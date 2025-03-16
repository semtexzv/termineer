//! Tool provider implementation for MCP servers

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::mcp::client::McpClient;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::protocol::Tool;

/// Tool provider for interacting with MCP servers
///
/// This manages a connection to an MCP server and provides
/// methods for listing and executing tools.
pub struct McpToolProvider {
    /// MCP client for communicating with the server
    client: McpClient,
    /// URL of the server
    #[allow(dead_code)]
    server_url: String,
    /// Available tools, cached for efficiency
    tools: Mutex<HashMap<String, Tool>>,
}

impl McpToolProvider {
    /* WebSocket-based connection removed in favor of file-based MCP configuration */
    
    /// Create a new tool provider for an MCP process with environment variables
    pub async fn new_process_with_env(
        name: &str, 
        executable: &str, 
        args: &[&str],
        env: &HashMap<String, String>
    ) -> McpResult<Self> {
        // Create client
        let client = McpClient::new();

        // Connect to process with environment variables
        client.connect_process_with_env(name, executable, args, env).await?;

        // Initialize client
        client
            .initialize(crate::mcp::protocol::ClientInfo {
                name: "Termineer".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            })
            .await?;

        // Create provider
        let provider = Self {
            client,
            server_url: format!("process://{}", executable),
            tools: Mutex::new(HashMap::new()),
        };

        // Refresh tools
        provider.refresh_tools().await?;

        Ok(provider)
    }

    /// Refresh the list of available tools
    pub async fn refresh_tools(&self) -> McpResult<()> {
        // List tools
        let tools = match self.client.list_tools().await {
            Ok(tools) => tools,
            Err(err) => {
                bprintln!(error: "Failed to refresh tools: {}", err);
                return Err(err);
            }
        };

        // Store tools by ID - now using a Mutex
        let mut tools_map = self.tools.lock().unwrap();
        tools_map.clear();
        for tool in tools {
            tools_map.insert(tool.name.clone(), tool);
        }

        Ok(())
    }

    /// List available tools
    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools.lock().unwrap().values().cloned().collect()
    }

    /// Get a tool by ID
    #[allow(dead_code)]
    pub fn get_tool(&self, id: &str) -> Option<Tool> {
        self.tools.lock().unwrap().get(id).cloned()
    }

    /// Execute a tool with the given arguments
    pub async fn execute_tool(
        &self,
        id: &str,
        arguments: Value,
    ) -> McpResult<crate::mcp::protocol::CallToolResult> {
        // Get the tool info or return an error if not found
        {
            let tools_map = self.tools.lock().unwrap();
            if !tools_map.contains_key(id) {
                return Err(McpError::ToolNotFound(id.to_string()));
            }
        };

        // Call tool and return the full result - removed verbose logging
        self.client.call_tool(id, arguments).await
    }

    /// Get content objects from a tool result
    pub async fn get_tool_content(
        &self,
        id: &str,
        arguments: Value,
    ) -> McpResult<Vec<crate::mcp::protocol::content::Content>> {
        // Execute the tool
        let result = self.execute_tool(id, arguments).await?;
        
        // Removed verbose result logging

        // Convert to content objects
        result.to_content_objects().map_err(|e| {
            McpError::ProtocolError(format!("Failed to parse tool result as content: {}", e))
        })
    }
}