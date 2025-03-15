//! MCP protocol messages and data structures

pub mod common;
pub mod content;
pub mod messages;
pub mod schema;
pub mod tools;

// Re-export common types for easier imports
pub use messages::{
    ClientCapabilities, ClientInfo, InitializeParams, InitializeResult, JsonRpcError,
    JsonRpcMessage, MessageContent, Request, RootsCapabilities, ServerInfo,
};
pub use tools::{CallToolParams, CallToolResult, ListToolsResult, Tool};
