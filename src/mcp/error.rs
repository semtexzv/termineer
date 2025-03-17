//! Error types for MCP client

use crate::mcp::protocol::JsonRpcError;
use thiserror::Error;

/// Standard JSON-RPC error codes as defined in the MCP specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ErrorCode {
    // SDK error codes
    ConnectionClosed = -32000,
    RequestTimeout = -32001,
    // Standard JSON-RPC error codes
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
}

impl ErrorCode {
    /// Convert from i32 to ErrorCode
    #[allow(dead_code)]
    pub fn from_i32(code: i32) -> Option<Self> {
        match code {
            -32000 => Some(Self::ConnectionClosed),
            -32001 => Some(Self::RequestTimeout),
            -32700 => Some(Self::ParseError),
            -32600 => Some(Self::InvalidRequest),
            -32601 => Some(Self::MethodNotFound),
            -32602 => Some(Self::InvalidParams),
            -32603 => Some(Self::InternalError),
            _ => None,
        }
    }

    /// Get a description for this error code
    pub fn description(&self) -> &'static str {
        match self {
            Self::ConnectionClosed => "Connection closed",
            Self::RequestTimeout => "Request timed out",
            Self::ParseError => "Parse error",
            Self::InvalidRequest => "Invalid request",
            Self::MethodNotFound => "Method not found",
            Self::InvalidParams => "Invalid parameters",
            Self::InternalError => "Internal error",
        }
    }
}

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

    /// Standard error with code
    #[error("{} ({0})", code.description())]
    #[allow(dead_code)]
    StandardError {
        code: ErrorCode,
        data: Option<String>,
    },
}

impl McpError {
    /// Create a standard error from an error code
    #[allow(dead_code)]
    pub fn standard(code: ErrorCode, data: Option<String>) -> Self {
        Self::StandardError { code, data }
    }

    /// Convert from JsonRpcError to McpError with better error code handling
    #[allow(dead_code)]
    pub fn from_json_rpc_error(error: JsonRpcError) -> Self {
        if let Some(code) = ErrorCode::from_i32(error.code) {
            Self::standard(code, error.data.map(|d| format!("{:?}", d)))
        } else {
            Self::JsonRpcError(error)
        }
    }
}

/// Result type for MCP operations
pub type McpResult<T> = Result<T, McpError>;
