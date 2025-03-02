//! Types for agent identification and messaging

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::mpsc;

/// Unique identifier for an agent
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Messages that can be sent to an agent
#[derive(Debug)]
pub enum AgentMessage {
    /// Regular user input to be processed
    UserInput(String),

    /// Special command for the agent
    Command(AgentCommand),

    /// Request to interrupt current operation
    Interrupt,

    /// Request to terminate the agent
    Terminate,
}

/// Commands for controlling agent behavior
#[derive(Debug)]
pub enum AgentCommand {
    /// Set the model to use
    SetModel(String),

    /// Enable or disable tools
    EnableTools(bool),

    /// Set the system prompt
    SetSystemPrompt(String),

    /// Reset the conversation
    ResetConversation,
}

/// Possible states of an agent
#[derive(Debug, Clone, PartialEq)]
pub enum AgentState {
    /// Agent is idle, waiting for input
    Idle,

    /// Agent is processing input or generating a response
    Processing,

    /// Agent is running a tool
    RunningTool { tool: String, interruptible: bool },

    /// Agent has been terminated
    Terminated,
}

/// Errors that can occur when interacting with agents
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    AgentNotFound(AgentId),

    #[error("Failed to deliver message to agent")]
    MessageDeliveryFailed,

    #[error("Agent terminated")]
    Terminated,

    #[error("Timeout while waiting for agent to terminate")]
    TerminationTimeout,

    #[error("Failed to create agent: {0}")]
    CreationFailed(String),
}

/// Type alias for an agent message sender
pub type AgentSender = mpsc::Sender<AgentMessage>;

/// Type alias for an agent message receiver
pub type AgentReceiver = mpsc::Receiver<AgentMessage>;