//! Model Context Protocol (MCP) client implementation

pub mod protocol;
pub mod error;
pub mod connection;
pub mod client;
pub mod tool_provider;

// We'll avoid re-exporting types that aren't used externally