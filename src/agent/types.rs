//! Types for agent identification and messaging

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::{mpsc, watch};

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

/// Simple state representation that can be stored in an atomic value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AgentStateCode {
    Idle = 0,
    Processing = 1,
    RunningTool = 2,
    Terminated = 3,
    Done = 4,
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
    
    /// Agent has completed its task (done tool used)
    Done,
}

impl AgentState {
    /// Convert to the simple state code
    pub fn to_code(&self) -> AgentStateCode {
        match self {
            AgentState::Idle => AgentStateCode::Idle,
            AgentState::Processing => AgentStateCode::Processing,
            AgentState::RunningTool { .. } => AgentStateCode::RunningTool,
            AgentState::Terminated => AgentStateCode::Terminated,
            AgentState::Done => AgentStateCode::Done,
        }
    }
    
    /// Get a readable string representation of the state
    pub fn as_display_string(&self) -> String {
        match self {
            AgentState::Idle => "Ready".to_string(),
            AgentState::Processing => "Thinking...".to_string(),
            AgentState::RunningTool { tool, .. } => format!("Running: {}", tool),
            AgentState::Terminated => "Terminated".to_string(),
            AgentState::Done => "Task completed".to_string(),
        }
    }
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

pub type StateSender = watch::Sender<AgentState>;
pub type StateReceiver = watch::Receiver<AgentState>;