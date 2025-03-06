//! Error types for MCP client

use crate::mcp::protocol::JsonRpcError;
use thiserror::Error;

/// Errors that can occur in the MCP client
#[derive(Error, Debug)]
pub enum McpError {
    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// JSON-RPC error
    #[error("JSON-RPC error: {0}")]
    JsonRpcError(JsonRpcError),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    /// Response not received
    #[error("Response not received")]
    ResponseNotReceived,

    /// Server disconnected
    #[error("Server disconnected")]
    ServerDisconnected,

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(String),
}

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;
