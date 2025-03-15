pub mod agent;
pub mod done;
pub mod mcp;
pub mod fetch;
pub mod patch;
pub mod path_utils;
pub mod read;
pub mod search;
pub mod shell;
pub mod task;
#[cfg(target_os = "macos")]
pub mod ui;
pub mod wait;
pub mod write;

// Re-export all tool functions
pub use agent::execute_agent_tool;
pub use done::execute_done;
pub use mcp::execute_dynamic_mcp_tool;
pub use fetch::execute_fetch;
pub use patch::execute_patch;
pub use read::execute_read;
pub use search::execute_search;
pub use shell::InterruptData;
pub use task::execute_task;
#[cfg(target_os = "macos")]
pub use ui::input::execute_input;
#[cfg(target_os = "macos")]
pub use ui::screendump::execute_screendump;
#[cfg(target_os = "macos")]
pub use ui::screenshot::execute_screenshot;
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

    /// Requested state change for the agent
    pub state_change: AgentStateChange,

    /// Content representing the tool's output as LLM content objects
    pub content: Vec<crate::llm::Content>,
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
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            success: true,
            state_change: AgentStateChange::Continue,
            content: vec![crate::llm::Content::Text { text: output.into() }],
        }
    }

    /// Create a successful tool result with multiple content items
    pub fn success_with_content(content: Vec<crate::llm::Content>) -> Self {
        Self {
            success: true,
            state_change: AgentStateChange::Continue,
            content,
        }
    }

    /// Create a successful tool result from MCP content
    pub fn success_from_mcp(mcp_content: Vec<crate::mcp::protocol::content::Content>) -> Self {
        use crate::mcp::protocol::content::McpContent;

        // Convert each MCP content to LLM content
        let llm_content = mcp_content
            .into_iter()
            .map(|c| c.to_llm_content())
            .collect();

        Self::success_with_content(llm_content)
    }

    /// Create a default tool result with continue state
    pub fn default(success: bool, output: String) -> Self {
        Self {
            success,
            state_change: AgentStateChange::Continue,
            content: vec![crate::llm::Content::Text { text: output }],
        }
    }

    /// Create an error tool result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            state_change: AgentStateChange::Continue,
            content: vec![crate::llm::Content::Text { text: message.into() }],
        }
    }

    /// Create an error tool result with a formatted message
    pub fn error_formatted(message: impl Into<String>) -> Self {
        let message = message.into();
        bprintln!(error: "{}", message);
        Self::error(message)
    }

    /// Create a tool result that puts the agent in waiting state
    pub fn wait(_reason: impl Into<String>) -> Self {
        Self {
            success: true,
            state_change: AgentStateChange::Wait,
            content: vec![crate::llm::Content::Text {
                text: "Resumed".to_string(),
            }],
        }
    }

    /// Create a tool result that marks the agent as done
    pub fn done(summary: impl Into<String>) -> Self {
        let summary_string = summary.into();
        Self {
            success: true,
            state_change: AgentStateChange::Done,
            content: vec![crate::llm::Content::Text { text: summary_string }],
        }
    }

    /// Get a text representation of the content
    pub fn to_text(&self) -> String {
        let mut result = String::new();
        for content in &self.content {
            match content {
                crate::llm::Content::Text { text } => {
                    result.push_str(text);
                    result.push('\n');
                }
                _ => {
                    result.push_str("[Complex content - see formatted response]\n");
                }
            }
        }
        result
    }
}

// Use macros for output instead of direct functions

use crate::agent::AgentId;

/// Handles tool execution with consistent processing
pub struct ToolExecutor {
    /// Whether tools are in read-only mode
    readonly_mode: bool,
    /// Whether to suppress console output
    silent_mode: bool,
    /// ID of the agent that owns this tool executor
    agent_id: Option<AgentId>,
    /// List of tools that are specifically disabled
    disabled_tools: Vec<String>,
}

impl ToolExecutor {
    /// Create a new tool executor
    pub fn new(readonly_mode: bool, silent_mode: bool) -> Self {
        Self {
            readonly_mode,
            silent_mode,
            agent_id: None,
            disabled_tools: Vec::new(),
        }
    }

    /// Create a new tool executor with agent ID
    pub fn with_agent_id(readonly_mode: bool, silent_mode: bool, agent_id: AgentId) -> Self {
        Self {
            readonly_mode,
            silent_mode,
            agent_id: Some(agent_id),
            disabled_tools: Vec::new(),
        }
    }
    
    /// Set the list of disabled tools
    pub fn set_disabled_tools(&mut self, disabled_tools: Vec<String>) {
        self.disabled_tools = disabled_tools;
    }

    /// Check if executor is in silent mode
    pub fn is_silent(&self) -> bool {
        self.silent_mode
    }
    
    /// Check if a specific tool is disabled
    fn is_tool_disabled(&self, tool_name: &str) -> bool {
        self.disabled_tools
            .iter()
            .any(|disabled| disabled.trim().to_lowercase() == tool_name.trim().to_lowercase())
    }
    
    /// Execute a tool based on name, args, and body provided by the LLM
    pub async fn execute_with_parts(&self, tool_name: &str, args: &str, body: &str) -> ToolResult {
        // Using pre-parsed components directly
        let tool_name = tool_name.trim().to_lowercase();

        // Check if the tool is specifically disabled
        if self.is_tool_disabled(&tool_name) {
            if !self.silent_mode {
                bprintln !(error:"Tool '{}' has been disabled by user configuration", tool_name);
            }
            return ToolResult::error(format!(
                "Tool '{}' has been disabled by user configuration",
                tool_name
            ));
        }

        // In readonly mode, only allow read-only tools (and task which will create readonly subagents)
        if self.readonly_mode && !self.is_readonly_tool(&tool_name) {
            if !self.silent_mode {
                // Always use buffer-based printing with direct formatting
                bprintln !(error:"Tool '{}' is not available in read-only mode", tool_name);
            }
            return ToolResult::error(format!(
                "Tool '{}' is not available in read-only mode",
                tool_name
            ));
        }

        // Execute the appropriate tool with silent mode flag. Shell handled externally
        let mut result = match tool_name.as_str() {
            "agent" => execute_agent_tool(args, body, self.silent_mode, self.agent_id).await,
            "read" => execute_read(args, body, self.silent_mode).await,
            "write" => execute_write(args, body, self.silent_mode).await,
            "patch" => execute_patch(args, body, self.silent_mode).await,
            "fetch" => execute_fetch(args, body, self.silent_mode).await,
            "search" => execute_search(args, body, self.silent_mode).await,
            #[cfg(target_os = "macos")]
            "screenshot" => execute_screenshot(args, body, self.silent_mode).await,
            #[cfg(target_os = "macos")]
            "input" => execute_input(args, body, self.silent_mode).await,
            "done" => execute_done(args, body, self.silent_mode),
            "task" => execute_task(args, body, self.silent_mode, self.agent_id).await,
            #[cfg(target_os = "macos")]
            "screendump" => execute_screendump(args, body, self.silent_mode).await,
            "wait" => execute_wait(args, body, self.silent_mode),
            _ => {
                // Check if tool_name is an MCP server name
                if crate::mcp::has_provider(&tool_name) {
                    // In readonly mode, MCP tools are not available for safety
                    if self.readonly_mode {
                        if !self.silent_mode {
                            bprintln!(error: "MCP tool '{}' is not available in read-only mode", tool_name);
                        }
                        return ToolResult::error(format!(
                            "MCP tool '{}' is not available in read-only mode",
                            tool_name
                        ));
                    }
                    
                    // It's an MCP server name, so handle it as a dynamic MCP tool
                    execute_dynamic_mcp_tool(&tool_name, args, body, self.silent_mode).await
                } else {
                    if !self.silent_mode {
                        // Always use buffer-based printing with direct formatting
                        bprintln !(error:"Unknown tool: {:?}, args:{}, body:{}", tool_name, args, body);
                    }
                    ToolResult::error(format!("Unknown tool: {:?}", tool_name))
                }
            }
        };

        // Apply UTF-8 safe truncation to long text outputs
        for i in 0..result.content.len() {
            if let crate::llm::Content::Text { text } = &result.content[i] {
                if text.len() > crate::constants::MAX_TOOL_OUTPUT_LENGTH {
                    let original_length = text.len();

                    // Apply truncation - use default parameters from constants
                    let truncated_text = truncate_utf8_content(text, None, None, None, None);

                    // Update the content with truncated text
                    result.content[i] = crate::llm::Content::Text {
                        text: truncated_text,
                    };

                    // Log truncation if not in silent mode
                    if !self.silent_mode {
                        // Get the new length after truncation
                        let new_length =
                            if let crate::llm::Content::Text { text } = &result.content[i] {
                                text.len()
                            } else {
                                0 // Should never happen
                            };

                        let truncated_bytes = original_length - new_length;
                        let truncated_kb = truncated_bytes / 1024;

                        bprintln!(
                            "{}🔍 Truncated tool output from {} KB to {} KB (saved {} KB){}",
                            crate::constants::FORMAT_YELLOW,
                            original_length / 1024,
                            new_length / 1024,
                            truncated_kb,
                            crate::constants::FORMAT_RESET
                        );
                    }
                }
            }
        }

        result
    }

    /// Check if a tool is read-only
    fn is_readonly_tool(&self, name: &str) -> bool {
        matches!(
            name,
            "read"
                | "shell"
                | "asyncshell"
                | "asyncshell-list"
                | "asyncshell-kill"
                | "fetch"
                | "search"
                | "screenshot"
                | "screendump"
                | "done"
                | "task"
                | "agent"
                | "wait"
                | "computer"
                // Note: input is NOT read-only as it modifies application state
        )
    }
}

/// Truncates long string content while respecting UTF-8 character boundaries
///
/// This function handles proper truncation of tool outputs, ensuring that:
/// 1. UTF-8 character boundaries are preserved (no broken Unicode characters)
/// 2. A reasonable amount of content from the start is kept for context
/// 3. Optionally preserves content from the end (configurable)
/// 4. Inserts a placeholder to indicate truncation occurred
///
/// # Arguments
/// * `content` - The string content to truncate
/// * `max_length` - Maximum desired total length (default from constants)
/// * `start_length` - How much to preserve from the start
/// * `end_length` - How much to preserve from the end (0 to skip end preservation)
/// * `placeholder` - Text to insert between preserved parts
///
/// # Returns
/// Truncated string with placeholder if needed, or original string if short enough
pub fn truncate_utf8_content(
    content: &str,
    max_length: Option<usize>,
    start_length: Option<usize>,
    end_length: Option<usize>,
    placeholder: Option<&str>,
) -> String {
    // Use provided values or defaults from constants
    let max_length = max_length.unwrap_or(crate::constants::MAX_TOOL_OUTPUT_LENGTH);
    let start_length = start_length.unwrap_or(crate::constants::PRESERVED_START_LENGTH);
    let end_length = if crate::constants::PRESERVE_OUTPUT_END {
        end_length.unwrap_or(crate::constants::PRESERVED_END_LENGTH)
    } else {
        0
    };
    let placeholder = placeholder.unwrap_or(crate::constants::TRUNCATION_PLACEHOLDER);

    // If content is already short enough, return as is
    if content.len() <= max_length {
        return content.to_string();
    }

    // Calculate how much we can preserve from start and end
    // We need to ensure the final output fits within max_length
    let total_preserved = start_length + end_length + placeholder.len();
    let (actual_start, actual_end) = if total_preserved > max_length {
        // If the start, end, and placeholder combined exceed the max length,
        // proportionally reduce start and end to fit
        let available = max_length.saturating_sub(placeholder.len());
        let start_ratio = start_length as f64 / (start_length + end_length) as f64;
        let actual_start = (available as f64 * start_ratio) as usize;
        let actual_end = available.saturating_sub(actual_start);
        (actual_start, actual_end)
    } else {
        (start_length, end_length)
    };

    // Find valid UTF-8 character boundary for start section
    let prefix_boundary = if actual_start > 0 {
        content
            .char_indices()
            .take_while(|(idx, _)| *idx < actual_start)
            .last()
            .map(|(idx, c)| idx + c.len_utf8())
            .unwrap_or(0)
    } else {
        0
    };

    // Build result with or without end content
    if actual_end > 0 {
        // Find valid UTF-8 character boundary for end section
        let suffix_start = content
            .char_indices()
            .rev()
            .take_while(|(idx, _)| content.len().saturating_sub(*idx) <= actual_end)
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(content.len());

        // Combine start, placeholder, and end
        format!(
            "{}{}{}",
            &content[..prefix_boundary],
            placeholder,
            &content[suffix_start..]
        )
    } else {
        // Just use the start and placeholder
        format!("{}{}", &content[..prefix_boundary], placeholder)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_truncation() {
        // Test with ASCII content
        let ascii = "A".repeat(10_000);
        let truncated_ascii =
            truncate_utf8_content(&ascii, Some(1000), Some(100), Some(100), Some("[...]"));

        // Verify lengths
        assert!(truncated_ascii.len() < ascii.len());
        assert!(truncated_ascii.contains("[...]"));

        // Verify UTF-8 validity
        assert!(std::str::from_utf8(truncated_ascii.as_bytes()).is_ok());

        // Test with multi-byte UTF-8 characters (emoji - 4 bytes each)
        let emoji = "😀".repeat(1_000);
        let truncated_emoji =
            truncate_utf8_content(&emoji, Some(500), Some(100), Some(100), Some("[...]"));

        // Verify proper truncation
        assert!(truncated_emoji.len() < emoji.len());
        assert!(truncated_emoji.contains("[...]"));

        // Most importantly, verify UTF-8 validity is maintained
        assert!(std::str::from_utf8(truncated_emoji.as_bytes()).is_ok());

        // Test mixed content
        let mixed = format!("{}{}{}", "A".repeat(100), "😀".repeat(100), "Z".repeat(100));
        let truncated_mixed =
            truncate_utf8_content(&mixed, Some(200), Some(50), Some(50), Some("[...]"));

        // Verify truncation happened
        assert!(truncated_mixed.len() < mixed.len());
        assert!(truncated_mixed.contains("[...]"));

        // Verify both start and end sections are preserved
        assert!(truncated_mixed.starts_with(&"A".repeat(10)));
        assert!(truncated_mixed.ends_with(&"Z".repeat(10)));

        // Verify UTF-8 validity
        assert!(std::str::from_utf8(truncated_mixed.as_bytes()).is_ok());
    }
}
