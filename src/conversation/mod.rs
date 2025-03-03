//! Conversation management
//!
//! This module handles all aspects of conversation management including:
//! - Parsing and formatting of LLM responses
//! - Maintenance of conversation structure (removing empty messages)
//! - Truncation of conversations to stay within token limits
//! - Tool mapping to track relationships between tool calls and results
//! - Utility functions for conversation display and manipulation

mod maintenance;
mod truncation;

// Re-export all the components
pub use maintenance::{sanitize_conversation};
pub use truncation::{truncate_conversation, TruncationConfig};
use crate::constants::{FORMAT_GRAY, FORMAT_RESET};
use crate::llm::TokenUsage;
// Types and structs shared across conversation submodules can be defined here


/// Print the assistant's response to the output buffer
pub fn print_assistant_response(text: &str) {
    crate::bprintln!("{}", text);
}

/// Print token usage statistics to the output buffer
pub fn print_token_stats(usage: &TokenUsage) {
    crate::bprintln!(
        "{}{}[{} in / {} out] ({} read, {} written){}",
        FORMAT_GRAY,
        crate::constants::FORMAT_BOLD,
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_read_input_tokens,
        usage.cache_creation_input_tokens,
        FORMAT_RESET
    );
}

/// Check if a tool name is the "done" tool
pub fn is_done_tool(tool_name: &str) -> bool {
    tool_name == "done"
}