//! Model Context Protocol (MCP) client implementation

pub mod protocol;
pub mod error;
pub mod connection;
pub mod client;
pub mod tool_provider;

// Re-export common types for easier imports
pub use error::{McpError, McpResult};
pub use client::McpClient;