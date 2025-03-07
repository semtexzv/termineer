//! MCP client implementation

use crate::mcp::error::{McpError, McpResult};
use crate::mcp::process_connection::ProcessConnection;
use crate::mcp::protocol::{
    CallToolParams, CallToolResult, ClientCapabilities, ClientInfo, InitializeParams,
    InitializeResult, JsonRpcMessage, ListToolsResult, MessageContent, Request, RootsCapabilities,
    ServerInfo, Tool,
};
use crate::mcp::Connection;
use serde::Serialize;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// MCP client for communicating with MCP servers
pub struct McpClient {
    /// Connection to the MCP server (WebSocket or Process)
    connection: Arc<Mutex<Option<Box<dyn Connection>>>>,

    /// Counter for generating request IDs
    request_id: AtomicUsize,

    /// Whether the client has been initialized
    initialized: Arc<AtomicBool>,

    /// Server info received during initialization
    server_info: Arc<Mutex<Option<ServerInfo>>>,
}

/// Current MCP protocol version
const PROTOCOL_VERSION: &str = "2024-11-05";

impl McpClient {
    /// Create a new unconnected MCP client
    pub fn new() -> Self {
        Self {
            connection: Arc::new(Mutex::new(None)),
            request_id: AtomicUsize::new(1),
            initialized: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            server_info: Arc::new(Mutex::new(None)),
        }
    }

    /*
    /// Connect to an MCP server using WebSockets
    #[allow(dead_code)]
    pub async fn connect(&self, url: &str) -> McpResult<()> {
        // Create WebSocket connection
        let ws_conn = WebSocketConnection::connect(url).await?;

        // Store the connection
        let mut conn_guard = self.connection.lock().await;
        *conn_guard = Some(Box::new(ws_conn));

        Ok(())
    }
    */

    /// Connect to an MCP server using a subprocess
    pub async fn connect_process(
        &self,
        name: &str,
        executable: &str,
        args: &[&str],
    ) -> McpResult<()> {
        // Create process connection
        let proc_conn = ProcessConnection::spawn(name, executable, args).await?;

        // Store the connection
        let mut conn_guard = self.connection.lock().await;
        *conn_guard = Some(Box::new(proc_conn));

        Ok(())
    }

    /// Initialize the MCP client with the server
    pub async fn initialize(&self, client_info: ClientInfo) -> McpResult<InitializeResult> {
        // Check if connected
        if self.connection.lock().await.is_none() {
            return Err(McpError::ConnectionError("Not connected".to_string()));
        }

        // Check if already initialized
        if self.initialized.load(Ordering::SeqCst) {
            return Err(McpError::ProtocolError("Already initialized".to_string()));
        }

        // Create initialize parameters
        let params = InitializeParams {
            protocol_version: PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities {
                roots: RootsCapabilities { list_changed: true },
            },
            client_info,
        };
        // Send initialize request
        let result = self
            .send_request::<_, InitializeResult>("initialize", params)
            .await?;
        // Store server info
        let mut server_info_guard = self.server_info.lock().await;
        *server_info_guard = Some(result.server_info.clone());

        // Mark as initialized
        self.initialized.store(true, Ordering::SeqCst);
        Ok(result)
    }

    /// List available tools
    pub async fn list_tools(&self) -> McpResult<Vec<Tool>> {
        // Check if initialized
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(McpError::ProtocolError("Not initialized".to_string()));
        }

        // Send listTools request
        let result = self
            .send_request::<_, ListToolsResult>("tools/list", json!({}))
            .await?;

        Ok(result.tools)
    }

    /// Call a tool
    pub async fn call_tool(
        &self,
        tool_id: &str,
        arguments: serde_json::Value,
    ) -> McpResult<CallToolResult> {
        // Check if initialized
        if !self.initialized.load(Ordering::SeqCst) {
            return Err(McpError::ProtocolError("Not initialized".to_string()));
        }

        // Create callTool parameters
        let params = CallToolParams {
            name: tool_id.to_string(),
            arguments,
        };

        // Send callTool request and return the complete result
        self.send_request::<_, CallToolResult>("tools/call", params)
            .await
    }

    /// Get the server info if initialized
    #[allow(dead_code)]
    pub async fn server_info(&self) -> Option<ServerInfo> {
        let server_info_guard = self.server_info.lock().await;
        server_info_guard.clone()
    }

    /// Check if the client is connected
    #[allow(dead_code)]
    pub async fn is_connected(&self) -> bool {
        let conn_guard = self.connection.lock().await;
        conn_guard.is_some() // If we have a connection, we're connected
    }

    /// Check if the client is initialized
    #[allow(dead_code)]
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    /// Close the connection (just drop the connection)
    #[allow(dead_code)]
    pub async fn close(&self) -> McpResult<()> {
        let mut conn_guard = self.connection.lock().await;
        *conn_guard = None;
        Ok(())
    }

    /// Send a request and parse the response
    async fn send_request<P: Serialize, R: for<'de> serde::Deserialize<'de>>(
        &self,
        method: &str,
        params: P,
    ) -> McpResult<R> {
        // Generate request ID
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        // Create request message
        let params_value = serde_json::to_value(params)?;
        
        // Remove protocol-level logging completely
        
        let message = JsonRpcMessage {
            jsonrpc: "2.0".to_string(),
            id: Some(serde_json::Value::Number(serde_json::Number::from(id))),
            content: MessageContent::Request(Request {
                method: method.to_string(),
                params: Some(params_value),
            }),
        };

        // Get connection and send request
        let conn_guard = self.connection.lock().await;
        let conn = conn_guard
            .as_ref()
            .ok_or_else(|| McpError::ConnectionError("Not connected".to_string()))?;

        // Send request and get response
        let response = conn.send_message(message).await?;
        
        // Parse response
        match response.content {
            MessageContent::Response(resp) => {
                // Remove protocol-level logging completely
                
                // Parse result
                let result: R = serde_json::from_value(resp.result).unwrap();
                Ok(result)
            }
            MessageContent::Error(err) => {
                bprintln!(error: "MCP Error ({}): {:?}", method, err);
                Err(McpError::JsonRpcError(err.error))
            }
            _ => Err(McpError::ProtocolError(format!(
                "Unexpected response type for method {}",
                method
            ))),
        }
    }
}

impl Default for McpClient {
    fn default() -> Self {
        Self::new()
    }
}
