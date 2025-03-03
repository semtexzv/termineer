//! Conversation management
//!
//! This module handles all aspects of conversation management including:
//! - Parsing and formatting of LLM responses
//! - Maintenance of conversation structure (removing empty messages)
//! - Truncation of conversations to stay within token limits
//! - Tool mapping to track relationships between tool calls and results
//! - Utility functions for conversation display and manipulation

mod maintenance;
mod parse;
mod truncation;

// Re-export all the components
pub use maintenance::{sanitize_conversation};
pub use parse::{parse_assistant_response, print_assistant_response, print_token_stats};
pub use truncation::{truncate_conversation, TruncationConfig};

// Types and structs shared across conversation submodules can be defined here