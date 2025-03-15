//! Model Context Protocol (MCP) client implementation

pub mod error;
pub mod protocol;
// pub mod connection; // Removed WebSocket connection module
pub mod client;
pub mod config;
pub mod connection_trait;
pub mod manager;
pub mod process_connection;
pub mod tool_provider;

// Re-export types that are used externally
pub use connection_trait::Connection;

// Re-export manager functions for easy access
pub use manager::{
    register_provider,
    has_provider, 
    get_provider,
    get_provider_names,
    has_providers,
    get_mcp_tools_for_prompt,
    add_mcp_tools_to_prompt
};
