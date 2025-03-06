//! Agent module for handling conversations with LLM backends
//!
//! This module contains agent-related functionality including:
//! - Core Agent implementation
//! - Agent Manager
//! - Agent types and communication
//! - Interrupt handling

// Re-export the agent submodules
mod agent;
mod interrupt;
mod manager;
pub mod types;
// Re-export public types from the submodules
// Agent isn't publicly used, so we don't re-export it
pub use manager::AgentManager;
pub use types::{AgentId, AgentMessage, AgentReceiver, AgentState};
