//! Manager for multiple agent instances

use std::collections::HashMap;
use tokio::task::JoinHandle;
use tokio::sync::{mpsc, watch};
use std::sync::atomic::{AtomicU8, Ordering};

use super::agent::Agent;
use super::types::{AgentError, AgentId, AgentMessage, AgentSender, AgentState, AgentStateCode, StateReceiver, 
    InterruptSender, InterruptReceiver, InterruptSignal};
use crate::config::Config;
use crate::output::{SharedBuffer, CURRENT_BUFFER};

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

/// Manager for multiple agent instances
pub struct AgentManager {
    /// Map of agent ID to agent handle
    agents: HashMap<AgentId, AgentHandle>,

    /// Next agent ID to assign
    next_id: u64,
}

impl AgentManager {
    /// Create a new agent manager
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
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

        // Create the agent with both channels
        let agent = match Agent::new(id, name.clone(), config, receiver, interrupt_receiver, state_sender) {
            Ok(agent) => agent,
            Err(e) => return Err(AgentError::CreationFailed(e.to_string())),
        };

        // Spawn agent as a task with its own buffer
        let join_handle = spawn_agent_task(agent, buffer.clone());

        // Create and store handle with both senders
        let handle = AgentHandle {
            id,
            name,
            sender,
            interrupt_sender,
            join_handle,
            buffer,
            state
        };

        self.agents.insert(id, handle);

        Ok(id)
    }

    /// Get a list of all agents
    pub fn list_agents(&self) -> Vec<(AgentId, String)> {
        self.agents
            .iter()
            .map(|(id, handle)| (*id, handle.name.clone()))
            .collect()
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
            return Ok(handle.buffer.clone())
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
    
    /// Interrupt an agent through the dedicated interrupt channel
    pub fn interrupt_agent(&self, id: AgentId) -> Result<(), AgentError> {
        if let Some(handle) = self.agents.get(&id) {
            // Send through the dedicated interrupt channel with optional reason
            handle
                .interrupt_sender
                .try_send(InterruptSignal::new(Some("User requested interruption".to_string())))
                .map_err(|_| AgentError::MessageDeliveryFailed)?;
            Ok(())
        } else {
            Err(AgentError::AgentNotFound(id))
        }
    }
    
    /// Interrupt an agent with specific reason
    pub fn interrupt_agent_with_reason(&self, id: AgentId, reason: String) -> Result<(), AgentError> {
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
            let _ = handle.interrupt_sender.try_send(InterruptSignal::new(
                Some("Agent terminating".to_string())
            ));
            
            // Then send terminate message
            let _ = handle.sender.try_send(AgentMessage::Terminate);

            // During shutdown, don't wait for the task to complete
            // Just abort it to avoid any issues with buffer access
            handle.join_handle.abort();

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
            let _ = handle.interrupt_sender.try_send(InterruptSignal::new(
                Some("Agent terminating".to_string())
            ));
            
            // Then send terminate message
            let _ = handle.sender.try_send(AgentMessage::Terminate);

            // Abort the task
            handle.join_handle.abort();
        }
    }
}

/// Spawn an agent as a tokio task with its own buffer
fn spawn_agent_task(agent: Agent, buffer: SharedBuffer) -> JoinHandle<()> {
    tokio::spawn(CURRENT_BUFFER.scope(buffer, async move {
        agent.run().await;
    }))
}