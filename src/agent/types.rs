//! Types for agent identification and messaging

use serde::{Deserialize, Serialize};
use std::fmt;
use tokio::sync::{mpsc, watch};

/// Dedicated types for the interrupt channel
pub type InterruptSender = mpsc::Sender<InterruptSignal>;
pub type InterruptReceiver = mpsc::Receiver<InterruptSignal>;

/// Signal sent through the interrupt channel
#[derive(Debug, Clone)]
pub struct InterruptSignal {
    /// Optional reason for interruption
    pub reason: Option<String>,
}

impl InterruptSignal {
    /// Create a new interrupt signal
    pub fn new(reason: Option<String>) -> Self {
        Self { reason }
    }
}

/// Unique identifier for an agent
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
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

    /// Message from another agent with source information
    #[allow(dead_code)]
    AgentInput {
        /// Content of the message
        content: String,
        /// ID of the source agent
        source_id: AgentId,
        /// Name of the source agent
        source_name: String,
    },

    /// Special command for the agent
    Command(AgentCommand),

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

    /// Set the thinking budget in tokens
    SetThinkingBudget(usize),
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
    /// Optionally includes the final response from the agent
    Done(Option<String>),
}

impl AgentState {
    /// Get a readable string representation of the state
    pub fn as_display_string(&self) -> String {
        match self {
            AgentState::Idle => "Ready".to_string(),
            AgentState::Processing => "Thinking...".to_string(),
            AgentState::RunningTool { tool, .. } => format!("Running: {}", tool),
            AgentState::Terminated => "Terminated".to_string(),
            AgentState::Done(_) => "Task completed".to_string(),
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
    #[allow(dead_code)]
    TerminationTimeout,

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Failed to create agent: {0}")]
    CreationFailed(String),

    #[error("Error generating or retrieving response")]
    ResponseGenerationError,
}

/// Type alias for an agent message sender
pub type AgentSender = mpsc::Sender<AgentMessage>;

/// Type alias for an agent message receiver
pub type AgentReceiver = mpsc::Receiver<AgentMessage>;

pub type StateSender = watch::Sender<AgentState>;
pub type StateReceiver = watch::Receiver<AgentState>;
