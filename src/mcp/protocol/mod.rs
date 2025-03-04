//! MCP protocol messages and data structures

pub mod messages;
pub mod tools;

// Re-export common types for easier imports
pub use messages::{
    JsonRpcMessage, MessageContent, Request, Response, 
    Notification, ErrorResponse, JsonRpcError,
    ClientInfo, ClientCapabilities, RootsCapabilities,
    InitializeParams, InitializeResult, ServerInfo
};
pub use tools::{
    Tool, ListToolsParams, ListToolsResult,
    CallToolParams, CallToolResult
};