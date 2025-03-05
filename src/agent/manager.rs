//! Manager for multiple agent instances

use super::agent::Agent;
use super::types::{
    AgentError, AgentId, AgentMessage, AgentSender, AgentState, InterruptReceiver,
    InterruptSender, InterruptSignal, StateReceiver,
};
use crate::agent::AgentReceiver;
use crate::config::Config;
use crate::output::{SharedBuffer, CURRENT_BUFFER};
use std::collections::HashMap;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

/// Handle to an agent task
pub struct AgentHandle {
    /// Agent's unique identifier
    pub id: AgentId,

    /// Agent's name
    pub name: String,

    /// Channel for sending messages to the agent
    pub sender: AgentSender,

    /// Channel for sending high-priority interrupt signals
    pub interrupt_sender: InterruptSender,

    /// Tokio task handle for the agent
    pub join_handle: JoinHandle<()>,

    /// Buffer of this agent (for display purposes)
    pub buffer: SharedBuffer,

    /// State of this agent
    pub state: StateReceiver,
}

// No external synchronization primitives needed

/// Manager for multiple agent instances
///
/// Provides capabilities for managing multiple agents, including:
/// - Creating and terminating agents
/// - Looking up agents by ID or name
/// - Sending messages and interruption signals
/// - Tracking agent state and status
///
/// The manager maintains two indices:
/// 1. A primary index by AgentId for fast ID-based lookups
/// 2. A secondary index by name for convenient name-based lookups
pub struct AgentManager {
    /// Map of agent ID to agent handle (primary index)
    agents: HashMap<AgentId, AgentHandle>,
    
    /// Map of agent name to agent ID for efficient name lookups (secondary index)
    name_index: HashMap<String, AgentId>,

    /// Next agent ID to assign
    next_id: u64,
}

impl AgentManager {
    /// Create a new agent manager
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
            name_index: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new agent
    pub fn create_agent(&mut self, name: String, config: Config) -> Result<AgentId, AgentError> {
        // Create message channel for this agent
        let (sender, receiver) = mpsc::channel(100);

        // Create dedicated interrupt channel
        let (interrupt_sender, interrupt_receiver) = mpsc::channel(10);

        let (state_sender, state) = watch::channel(AgentState::Idle);

        // Generate unique ID
        let id = AgentId(self.next_id);
        self.next_id += 1;

        let buffer = SharedBuffer::new(100);

        // Create the agent with state channel
        let agent = match Agent::new(id, name.clone(), config, state_sender) {
            Ok(agent) => agent,
            Err(e) => return Err(AgentError::CreationFailed(e.to_string())),
        };

        // Spawn agent as a task with its own buffer
        let join_handle = spawn_agent_task(agent, buffer.clone(), receiver, interrupt_receiver);

        // Create and store handle with both senders
        let handle = AgentHandle {
            id,
            name,
            sender,
            interrupt_sender,
            join_handle,
            buffer,
            state,
        };

        // Store the name in the index first
        self.name_index.insert(handle.name.clone(), id);
        
        // Then store the agent handle with its ID
        self.agents.insert(id, handle);

        Ok(id)
    }

    /// Send a message to an agent
    pub fn send_message(&self, id: AgentId, message: AgentMessage) -> Result<(), AgentError> {
        if let Some(handle) = self.agents.get(&id) {
            handle
                .sender
                .try_send(message)
                .map_err(|_| AgentError::MessageDeliveryFailed)?;
            Ok(())
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }

    pub fn get_agent_buffer(&self, id: AgentId) -> Result<SharedBuffer, AgentError> {
        if let Some(handle) = self.agents.get(&id) {
            return Ok(handle.buffer.clone());
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }
    
    /// Get the current state of an agent
    pub fn get_agent_state(&self, id: AgentId) -> Result<AgentState, AgentError> {
        if let Some(handle) = self.agents.get(&id) {
            Ok(handle.state.borrow().clone())
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }

    /// Get a reference to an agent handle by ID
    pub fn get_agent_handle(&self, id: AgentId) -> Option<&AgentHandle> {
        self.agents.get(&id)
    }
    
    /// Get a mutable reference to an agent handle by ID
    pub fn get_agent_handle_mut(&mut self, id: AgentId) -> Option<&mut AgentHandle> {
        self.agents.get_mut(&id)
    }
    
    /// Get a list of all agents with their IDs and names
    pub fn get_agents(&self) -> Vec<(AgentId, String)> {
        self.agents
            .iter()
            .map(|(id, handle)| (*id, handle.name.clone()))
            .collect()
    }
    
    /// Get an agent ID by name
    /// Returns None if no agent with that name exists
    pub fn get_agent_id_by_name(&self, name: &str) -> Option<AgentId> {
        self.name_index.get(name).copied()
    }

    /// Interrupt an agent through the dedicated interrupt channel
    pub fn interrupt_agent(&self, id: AgentId) -> Result<(), AgentError> {
        if let Some(handle) = self.agents.get(&id) {
            // Send through the dedicated interrupt channel with optional reason
            handle
                .interrupt_sender
                .try_send(InterruptSignal::new(Some(
                    "User requested interruption".to_string(),
                )))
                .map_err(|_| AgentError::MessageDeliveryFailed)?;
            Ok(())
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }

    /// Interrupt an agent with specific reason
    pub fn interrupt_agent_with_reason(
        &self,
        id: AgentId,
        reason: String,
    ) -> Result<(), AgentError> {
        if let Some(handle) = self.agents.get(&id) {
            handle
                .interrupt_sender
                .try_send(InterruptSignal::new(Some(reason)))
                .map_err(|_| AgentError::MessageDeliveryFailed)?;
            Ok(())
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }

    /// Terminate an agent
    pub async fn terminate_agent(&mut self, id: AgentId) -> Result<(), AgentError> {
        if let Some(handle) = self.agents.remove(&id) {
            // Send interrupt signal first to stop any tool execution
            let _ = handle
                .interrupt_sender
                .try_send(InterruptSignal::new(Some("Agent terminating".to_string())));

            // Then send terminate message
            let _ = handle.sender.try_send(AgentMessage::Terminate);

            // During shutdown, don't wait for the task to complete
            // Just abort it to avoid any issues with buffer access
            handle.join_handle.abort();
            
            // Remove from name index
            self.name_index.remove(&handle.name);

            Ok(())
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }

    /// Terminate all agents
    pub async fn terminate_all(&mut self) {
        // Don't collect ids first - just directly handle all agents
        // This avoids any issues with buffer access during termination
        for (_id, handle) in self.agents.drain() {
            // Send interrupt signal first to stop any tool execution
            let _ = handle
                .interrupt_sender
                .try_send(InterruptSignal::new(Some("Agent terminating".to_string())));

            // Then send terminate message
            let _ = handle.sender.try_send(AgentMessage::Terminate);

            // Abort the task
            handle.join_handle.abort();
        }
        
        // Clear the name index
        self.name_index.clear();
    }
}

/// Spawn an agent as a tokio task with its own buffer
fn spawn_agent_task(
    agent: Agent,
    buffer: SharedBuffer,
    agent_receiver: AgentReceiver,
    interrupt_receiver: InterruptReceiver,
) -> JoinHandle<()> {
    tokio::spawn(CURRENT_BUFFER.scope(buffer, async move {
        // Pass None since we now use the global agent manager
        agent.run(agent_receiver, interrupt_receiver).await;
    }))
}