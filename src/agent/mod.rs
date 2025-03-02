//! Agent module for handling conversations with LLM backends
//!
//! This module contains agent-related functionality including:
//! - Core Agent implementation
//! - Agent Manager
//! - Agent types and communication
//! - Interrupt handling

// Re-export the agent submodules
mod agent;
mod manager;
pub mod types;
mod interrupt;

// Re-export public types from the submodules
pub use agent::Agent;
pub use manager::AgentManager;
pub use types::{AgentCommand, AgentError, AgentId, AgentMessage, AgentReceiver, AgentSender, AgentState};