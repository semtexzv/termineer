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
mod tool_mapping;
mod truncation;

// Re-export all the components
pub use maintenance::{is_empty_content, sanitize_conversation};
pub use parse::{is_done_tool, parse_assistant_response, print_assistant_response, print_token_stats, ParsedResponse};
pub use tool_mapping::{ToolMapper, ToolInteraction};
pub use truncation::{truncate_conversation, TruncationConfig, TruncationResult};

// Types and structs shared across conversation submodules can be defined here