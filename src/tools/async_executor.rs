use crate::tools::ToolResult;

/// Message types for asynchronous tool execution
pub enum ToolMessage {
    /// A line of output from the tool
    Line(String),
    /// Completion signal with final result
    Complete(ToolResult),
}