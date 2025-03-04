//! Error types for MCP client

use thiserror::Error;
use crate::mcp::protocol::JsonRpcError;

/// Errors that can occur in the MCP client
#[derive(Error, Debug)]
pub enum McpError {
    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    /// WebSocket error
    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),
    
    /// JSON-RPC error
    #[error("JSON-RPC error: {0}")]
    JsonRpcError(JsonRpcError),
    
    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    /// Timeout error
    #[error("Request timed out")]
    Timeout,
    
    /// Response not received
    #[error("Response not received")]
    ResponseNotReceived,
    
    /// Server disconnected
    #[error("Server disconnected")]
    ServerDisconnected,

    /// Tool not found
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Tool execution error 
    #[error("Tool execution error: {0}")]
    ToolExecutionError(String),
    
    /// Initialization error
    #[error("Initialization error: {0}")]
    InitializationError(String),

    /// Protocol error
    #[error("Protocol error: {0}")]
    ProtocolError(String),

    /// URL parsing error
    #[error("URL parsing error: {0}")]
    UrlError(#[from] url::ParseError),
}

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;