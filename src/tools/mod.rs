pub mod agent;
pub mod done;
pub mod fetch;
pub mod patch;
pub mod read;
pub mod search;
pub mod shell;
pub mod task;
pub mod wait;
pub mod write;

// Re-export all tool functions
pub use agent::execute_agent_tool;
pub use done::execute_done;
pub use fetch::execute_fetch;
pub use patch::execute_patch;
pub use read::execute_read;
pub use search::execute_search;
pub use shell::{InterruptData};
pub use task::execute_task;
pub use wait::execute_wait;
pub use write::execute_write;

/// Possible state changes that a tool can request for the agent
#[derive(Debug, Clone, PartialEq)]
pub enum AgentStateChange {
    /// Continue processing normally
    Continue,
    /// Put the agent in waiting state
    Wait,
    /// Mark the agent as done
    Done,
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Whether the tool execution was successful
    pub success: bool,

    /// Full output to send to the LLM
    pub agent_output: String,
    
    /// Requested state change for the agent
    pub state_change: AgentStateChange,
}

// This allows backward compatibility with legacy code that doesn't specify state_change
impl Default for AgentStateChange {
    fn default() -> Self {
        AgentStateChange::Continue
    }
}

impl ToolResult {
    /// Create a successful tool result that continues processing
    #[allow(dead_code)]
    pub fn success(output: String) -> Self {
        Self {
            success: true,
            agent_output: output,
            state_change: AgentStateChange::Continue,
        }
    }
    
    /// Create a default tool result with continue state
    pub fn default(success: bool, agent_output: String) -> Self {
        Self {
            success,
            agent_output,
            state_change: AgentStateChange::Continue,
        }
    }

    /// Create an error tool result
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            agent_output: message,
            state_change: AgentStateChange::Continue,
        }
    }
    
    /// Create a tool result that puts the agent in waiting state
    pub fn wait(reason: String) -> Self {
        Self {
            success: true,
            agent_output: "Resumed".to_string(),
            state_change: AgentStateChange::Wait,
        }
    }
    
    /// Create a tool result that marks the agent as done
    pub fn done(summary: String) -> Self {
        Self {
            success: true,
            agent_output: summary,
            state_change: AgentStateChange::Done,
        }
    }
}

// Use macros for output instead of direct functions

use std::sync::{Arc, Mutex};
use crate::agent::AgentManager;
use crate::agent::AgentId;

/// Handles tool execution with consistent processing
pub struct ToolExecutor {
    /// Whether tools are in read-only mode
    readonly_mode: bool,
    /// Whether to suppress console output
    silent_mode: bool,
    /// ID of the agent that owns this tool executor
    agent_id: Option<AgentId>,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(readonly_mode: bool, silent_mode: bool) -> Self {
        Self {
            readonly_mode,
            silent_mode,
            agent_id: None,
        }
    }

    /// Create a new tool executor with agent ID
    pub fn with_agent_manager(readonly_mode: bool, silent_mode: bool, agent_id: AgentId) -> Self {
        // Keep the method signature for backward compatibility, but we'll use the global manager
        Self {
            readonly_mode,
            silent_mode,
            agent_id: Some(agent_id),
        }
    }
    
    /// Set the agent ID for this tool executor
    pub fn set_agent_id(&mut self, agent_id: AgentId) {
        self.agent_id = Some(agent_id);
    }

    /// Check if executor is in silent mode
    pub fn is_silent(&self) -> bool {
        self.silent_mode
    }

    /// Execute a tool based on name, args, and body provided by the LLM
    pub async fn execute_with_parts(&self, tool_name: &str, args: &str, body: &str) -> ToolResult {
        // Using pre-parsed components directly
        let tool_name = tool_name.trim().to_lowercase();

        // In readonly mode, only allow read-only tools (and task which will create readonly subagents)
        if self.readonly_mode && !self.is_readonly_tool(&tool_name) {
            if !self.silent_mode {
                // Always use buffer-based printing with direct formatting
                crate::berror_println!("Tool '{}' is not available in read-only mode", tool_name);
            }
            return ToolResult::error(format!(
                "Tool '{}' is not available in read-only mode",
                tool_name
            ));
        }

        // Execute the appropriate tool with silent mode flag. Shell handled externally
        let result = match tool_name.as_str() {
            "agent" => execute_agent_tool(args, body, self.silent_mode, self.agent_id).await,
            "read" => execute_read(args, body, self.silent_mode).await,
            "write" => execute_write(args, body, self.silent_mode).await,
            "patch" => execute_patch(args, body, self.silent_mode).await,
            "fetch" => execute_fetch(args, body, self.silent_mode).await,
            "search" => execute_search(args, body, self.silent_mode).await,
            "done" => execute_done(args, body, self.silent_mode),
            "task" => execute_task(args, body, self.silent_mode).await,
            "wait" => execute_wait(args, body, self.silent_mode),
            _ => {
                if !self.silent_mode {
                    // Always use buffer-based printing with direct formatting
                    crate::berror_println!("Unknown tool: {:?}", tool_name);
                }
                ToolResult::error(format!("Unknown tool: {:?}", tool_name))
            }
        };
        
        result
    }
    
    /// Check if a tool is read-only
    fn is_readonly_tool(&self, name: &str) -> bool {
        matches!(name, "read" | "shell" | "fetch" | "search" | "done" | "task" | "agent" | "wait")
    }
}
