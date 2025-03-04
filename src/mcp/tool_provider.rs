//! MCP tool provider for agent integration

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;

use crate::mcp::client::McpClient;
use crate::mcp::error::{McpError, McpResult};
use crate::mcp::protocol::{Tool, ClientInfo};

/// MCP tool provider for integrating MCP tools with the agent system
pub struct McpToolProvider {
    /// The MCP client
    client: Arc<McpClient>,
    
    /// Cache of available tools
    tool_cache: RwLock<HashMap<String, Tool>>,
    
    /// Server URL
    server_url: String,
}

/// Default timeout for tool execution
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

impl McpToolProvider {
    /// Create a new MCP tool provider and connect to the server
    pub async fn new(server_url: &str) -> McpResult<Self> {
        // Create client
        let client = McpClient::new();
        
        // Create provider
        let provider = Self {
            client: Arc::new(client),
            tool_cache: RwLock::new(HashMap::new()),
            server_url: server_url.to_string(),
        };
        
        // Connect to server
        provider.client.connect(server_url).await?;
        
        // Initialize client
        provider.client.initialize(ClientInfo {
            name: "autoswe".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }).await?;
        
        // Refresh tool cache
        provider.refresh_tools().await?;
        
        Ok(provider)
    }
    
    /// Refresh the tool cache
    pub async fn refresh_tools(&self) -> McpResult<()> {
        // List tools
        let tools = self.client.list_tools(None).await?;
        
        // Update cache
        let mut cache = self.tool_cache.write().await;
        *cache = tools.into_iter()
            .map(|t| (t.id.clone(), t))
            .collect();
        
        Ok(())
    }
    
    /// Get all available tools
    pub async fn list_tools(&self) -> Vec<Tool> {
        let cache = self.tool_cache.read().await;
        cache.values().cloned().collect()
    }
    
    /// Execute a tool call
    pub async fn execute_tool(&self, tool_id: &str, input: serde_json::Value) -> McpResult<serde_json::Value> {
        // Check if tool exists
        {
            let cache = self.tool_cache.read().await;
            if !cache.contains_key(tool_id) {
                return Err(McpError::ToolNotFound(tool_id.to_string()));
            }
        }
        
        // Execute with timeout
        match timeout(DEFAULT_TIMEOUT, self.client.call_tool(tool_id, input)).await {
            Ok(result) => result,
            Err(_) => Err(McpError::Timeout),
        }
    }
    
    /// Get a tool by ID
    pub async fn get_tool(&self, tool_id: &str) -> Option<Tool> {
        let cache = self.tool_cache.read().await;
        cache.get(tool_id).cloned()
    }
    
    /// Get the server URL
    pub fn server_url(&self) -> &str {
        &self.server_url
    }
    
    /// Returns true if connected to the server
    pub async fn is_connected(&self) -> bool {
        self.client.is_connected().await
    }
}