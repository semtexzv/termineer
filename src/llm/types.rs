//! Common types for LLM interactions
//!
//! These types are used across different LLM providers to
//! represent messages, content, and responses.

use crate::serde::element_array;
use serde::{Deserialize, Serialize};
 // Import JsonValue
 // Import HashMap

/// Response from an LLM provider
#[derive(Debug, Clone, PartialEq)] // Added Clone and PartialEq
pub struct LlmResponse {
    /// The content of the response
    pub content: Vec<Content>,

    /// Usage statistics, if available
    pub usage: Option<TokenUsage>,

    /// The stop sequence that terminated the response, if any
    pub stop_sequence: Option<String>,

    /// The reason the response was stopped (e.g., "max_tokens", "stop_sequence")
    pub stop_reason: Option<String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Deserialize, PartialEq)] // Added PartialEq
pub struct TokenUsage {
    /// Input tokens for the current request
    pub input_tokens: usize,

    /// Output tokens for the current request
    pub output_tokens: usize,

    pub cache_creation_input_tokens: usize,

    pub cache_read_input_tokens: usize,
}

/// Information about a message
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageInfo {
    /// User message
    User,

    /// Assistant message
    Assistant,

    /// System message
    System,

    /// Tool call
    ToolCall {
        tool_name: String,
        tool_index: Option<usize>,
    },

    /// Tool result
    ToolResult {
        tool_name: String,
        tool_index: Option<usize>,
    },

    /// Tool error
    ToolError {
        tool_name: String,
        tool_index: Option<usize>,
    },
}

/// Cache control information
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CacheControl {
    /// Message can be cached
    Ephemeral,
}

/// A message in a conversation
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    /// The sender role (user, assistant, system)
    pub role: String,

    /// The content of the message
    #[serde(with = "element_array")]
    pub content: Content,

    /// Additional information about the message
    pub info: MessageInfo,
}

impl Message {
    /// Create a new text message
    pub fn text(role: &str, content: String, info: MessageInfo) -> Self {
        Self {
            role: role.to_string(),
            content: Content::Text { text: content },
            info,
        }
    }

    #[allow(dead_code)]
    /// Create a new message with arbitrary content
    pub fn new(role: &str, content: Content, info: MessageInfo) -> Self {
        Self {
            role: role.to_string(),
            content,
            info,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Added PartialEq
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Base64 { media_type: String, data: String },
}
/// Content of a message
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Added PartialEq
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Content {
    /// Thinking (internal reasoning)
    Thinking {
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking: Option<String>,
    },

    /// Redacted thinking
    RedactedThinking {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<String>,
    },

    /// Text content
    Text { text: String },

    /// Image content
    Image { source: ImageSource },

    /// Document content
    Document { source: String },

}
