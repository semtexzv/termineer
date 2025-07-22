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
use crate::constants::{FORMAT_GRAY, FORMAT_RESET};
use crate::llm::TokenUsage;
pub use maintenance::sanitize_conversation;
pub use truncation::{TruncationConfig};
// Types and structs shared across conversation submodules can be defined here

/// Print the assistant's response to the output buffer
pub fn print_assistant_response(text: &str) {
    bprintln!("{text}");
}

/// Print token usage statistics to the output buffer
pub fn print_token_stats(usage: &TokenUsage) {
    bprintln!(
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
