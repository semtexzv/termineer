//! Common trait for MCP connections

use crate::mcp::error::McpResult;
use crate::mcp::protocol::JsonRpcMessage;
use async_trait::async_trait;

/// Common trait for MCP connections
#[async_trait]
pub trait Connection: Send + Sync {
    /// Send a message and wait for a response
    async fn send_message(&self, message: JsonRpcMessage) -> McpResult<JsonRpcMessage>;

    #[allow(dead_code)]
    /// Close the connection. Reserved for future use in dynamically connecting to servers
    async fn close(&self) -> McpResult<()>;

    #[allow(dead_code)]
    /// Check if the connection is still active
    fn is_connected(&self) -> bool;
}
