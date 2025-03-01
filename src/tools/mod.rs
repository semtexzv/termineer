pub mod shell;
pub mod read;
pub mod write;
pub mod patch;
pub mod done;
pub mod fetch;
pub mod task;

// Re-export all tool functions
pub use shell::execute_shell;
pub use read::execute_read;
pub use write::execute_write;
pub use patch::execute_patch;
pub use done::execute_done;
pub use fetch::execute_fetch;
pub use task::execute_task;

/// Result of executing a tool
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// Whether the tool execution was successful
    pub success: bool,

    /// Output to show to the user (possibly truncated)
    pub user_output: String,

    /// Full output to send to the LLM
    pub agent_output: String,
}

impl ToolResult {
    /// Create a successful tool result
    pub fn success(output: String) -> Self {
        Self {
            success: true,
            user_output: output.clone(),
            agent_output: output,
        }
    }

    /// Create a success result with different user and agent outputs
    pub fn success_with_agent_output(user_output: String, agent_output: String) -> Self {
        Self {
            success: true,
            user_output,
            agent_output,
        }
    }

    /// Create an error tool result
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            user_output: message.clone(),
            agent_output: message,
        }
    }
}

/// Handles tool execution with consistent processing
pub struct ToolExecutor {
    /// Whether tools are in read-only mode
    readonly_mode: bool,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(readonly_mode: bool) -> Self {
        Self {
            readonly_mode,
        }
    }

    /// Execute a tool based on content provided by the LLM
    pub fn execute(&self, tool_content: &str) -> ToolResult {
        // Parse the tool content into args (first line) and body (subsequent lines)
        let (tool_name, args, body) = self.parse_tool_content(tool_content);

        // In readonly mode, only allow read-only tools (and task which will create readonly subagents)
        if self.readonly_mode && !self.is_readonly_tool(&tool_name) {
            let error_msg = format!("Tool '{}' is not available in read-only mode", tool_name);
            return ToolResult::error(error_msg);
        }

        // Execute the appropriate tool
        let mut result = match tool_name.as_str() {
            "shell" => execute_shell(args, &body),
            "read" => execute_read(args, &body),
            "write" => execute_write(args, &body),
            "patch" => execute_patch(args, &body),
            "fetch" => execute_fetch(args, &body),
            "done" => execute_done(args, &body),
            "task" => execute_task(args, &body),
            _ => {
                let error_msg = format!("Unknown tool: {:?}", tool_name);
                ToolResult::error(error_msg)
            }
        };

        result
    }

    /// Parse tool content into name, args, and body
    fn parse_tool_content<'a>(&self, tool_content: &'a str) -> (String, &'a str, String) {
        // Split the tool content into args (first line) and body (subsequent lines)
        let mut lines = tool_content.trim().lines();
        let args_line = lines.next().unwrap_or("").trim();
        let body = lines.collect::<Vec<&str>>().join("\n");

        // Parse the tool name from the args line
        let parts: Vec<&str> = args_line.splitn(2, char::is_whitespace).collect();
        let tool_name = if !parts.is_empty() {
            parts[0].trim().to_lowercase()
        } else {
            "unknown".to_string()
        };

        let args = if parts.len() > 1 { parts[1] } else { "" };

        (tool_name, args, body)
    }

    /// Check if a tool is read-only
    fn is_readonly_tool(&self, name: &str) -> bool {
        matches!(name, "read" | "shell" | "fetch" | "done" | "task")
    }
}