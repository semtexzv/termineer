//! Agent module for handling conversations with LLM backends
//!
//! This module contains agent-related functionality including:
//! - Core Agent implementation
//! - Agent Manager
//! - Agent types and communication
//! - Interrupt handling

// Define submodules
mod agent_impl;
mod interrupt;
mod manager;
pub mod types;

// Re-export public types from the submodules
pub use types::{AgentId, AgentMessage, AgentReceiver, AgentState};

// Import manager implementation
use crate::config::Config;
use crate::output::SharedBuffer;
use lazy_static::lazy_static;
use manager::AgentManager;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Global agent manager available to all components
lazy_static! {
    static ref AGENT_MANAGER: Arc<Mutex<AgentManager>> = Arc::new(Mutex::new(AgentManager::new()));
}

// Public static methods for interacting with the agent manager

/// Create a new agent with the given name and configuration
pub fn create_agent(name: String, config: Config) -> Result<AgentId, types::AgentError> {
    let mut manager = AGENT_MANAGER.lock().unwrap();
    manager.create_agent(name, config)
}

/// Create a new agent with the given name, configuration, and buffer
pub fn create_agent_with_buffer(
    name: String,
    config: Config,
    buffer: SharedBuffer,
) -> Result<AgentId, types::AgentError> {
    let mut manager = AGENT_MANAGER.lock().unwrap();
    manager.create_agent_with_buffer(name, config, buffer)
}

/// Send a message to an agent
pub fn send_message(id: AgentId, message: AgentMessage) -> Result<(), types::AgentError> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.send_message(id, message)
}

/// Get the buffer for an agent
pub fn get_agent_buffer(id: AgentId) -> Result<SharedBuffer, types::AgentError> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.get_agent_buffer(id)
}

/// Get the current state of an agent
pub fn get_agent_state(id: AgentId) -> Result<AgentState, types::AgentError> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.get_agent_state(id)
}

/// Get a list of all agents with their IDs and names
pub fn get_agents() -> Vec<(AgentId, String)> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.get_agents()
}

/// Get an agent ID by name
pub fn get_agent_id_by_name(name: &str) -> Option<AgentId> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.get_agent_id_by_name(name)
}

/// Interrupt an agent
#[allow(dead_code)]
pub fn interrupt_agent(id: AgentId) -> Result<(), types::AgentError> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.interrupt_agent(id)
}

/// Interrupt an agent with a specific reason
pub fn interrupt_agent_with_reason(id: AgentId, reason: String) -> Result<(), types::AgentError> {
    let manager = AGENT_MANAGER.lock().unwrap();
    manager.interrupt_agent_with_reason(id, reason)
}

/// Terminate an agent
pub async fn terminate_agent(id: AgentId) -> Result<(), types::AgentError> {
    // Extract agent info before locking
    let agent_id = id;

    // Get a clone of the agent handle to send termination signals outside the lock
    let (interrupt_sender, sender) = {
        let manager = AGENT_MANAGER.lock().unwrap();
        if let Some(handle) = manager.get_agent_handle(agent_id) {
            (handle.interrupt_sender.clone(), handle.sender.clone())
        } else {
            return Err(types::AgentError::AgentNotFound(agent_id));
        }
    };

    // Send interrupt signal
    let _ = interrupt_sender.try_send(types::InterruptSignal::new(Some(
        "Agent terminating".to_string(),
    )));

    // Send terminate message
    let _ = sender.try_send(AgentMessage::Terminate);

    // Now remove from manager
    let mut manager = AGENT_MANAGER.lock().unwrap();
    manager.remove_agent(agent_id)
}

/// Terminate all agents
pub async fn terminate_all() {
    // Get all agents first
    let agents = get_agents();

    // Terminate each agent independently
    for (id, _) in agents {
        let _ = terminate_agent(id).await;
    }
}

/// Run an agent with a query until it completes and return the response
///
/// This function waits for the agent to reach the Done state with a response.
/// It relies on the agent properly setting its state to Done with the response
/// when it completes its task.
///
/// Parameters:
/// - agent_id: The ID of the agent to run
/// - query: The query to send to the agent
/// - timeout_seconds: Maximum time to wait for completion in seconds (default: 300)
///
/// Returns:
/// - The response from the agent if successful
pub async fn run_agent_to_completion(
    agent_id: AgentId,
    query: String,
    timeout_seconds: Option<u64>,
) -> Result<String, types::AgentError> {
    // Send the query to the agent
    send_message(agent_id, AgentMessage::UserInput(query))?;

    // Set timeout (default: 5 minutes)
    let timeout = Duration::from_secs(timeout_seconds.unwrap_or(300));
    let start_time = std::time::Instant::now();
    let mut last_polling_time = std::time::Instant::now();
    let polling_interval = Duration::from_millis(500);

    // Keep checking the agent state until it's done or timeout
    while start_time.elapsed() < timeout {
        // Only poll at the specified interval
        if last_polling_time.elapsed() >= polling_interval {
            last_polling_time = std::time::Instant::now();

            // Get the agent state
            let state = get_agent_state(agent_id)?;

            // Check if the agent is done
            if let AgentState::Done(Some(response)) = state {
                // Agent is done with a response
                return Ok(response);
            } else if let AgentState::Done(None) = state {
                // Agent is done but no response provided
                return Err(types::AgentError::ResponseGenerationError);
            } else if let AgentState::Terminated = state {
                // Agent was terminated
                return Err(types::AgentError::Terminated);
            }
        }

        // Small sleep to avoid tight loop
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // If we reached here, we timed out
    Err(types::AgentError::Timeout(format!(
        "Agent did not complete within {} seconds",
        timeout.as_secs()
    )))
}
