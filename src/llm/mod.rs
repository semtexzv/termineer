//! LLM provider abstraction layer
//! 
//! This module defines traits and types for interacting with
//! different LLM providers (Anthropic, etc.)

mod types;
pub mod anthropic;

use std::collections::BTreeSet;
pub use self::types::*;

/// Common trait for all LLM backends
pub trait Backend {
    /// Send a message to the LLM and get a response
    fn send_message(
        &self, 
        messages: &[Message], 
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
    ) -> Result<LlmResponse, LlmError>;
    
    /// Get the provider name
    fn name(&self) -> &str;
    
    /// Get the model name
    fn model(&self) -> &str;
}

/// Error types for LLM operations
#[derive(Debug)]
#[allow(dead_code)]
pub enum LlmError {
    /// API request error
    ApiError(String),
    
    /// Configuration error
    ConfigError(String),
    
    /// Rate limit error
    RateLimitError { retry_after: Option<u64> },
    
    /// Generic error
    Other(Box<dyn std::error::Error>),
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiError(msg) => write!(f, "API error: {}", msg),
            Self::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            Self::RateLimitError { retry_after } => {
                if let Some(seconds) = retry_after {
                    write!(f, "Rate limit exceeded. Retry after {} seconds", seconds)
                } else {
                    write!(f, "Rate limit exceeded")
                }
            },
            Self::Other(err) => write!(f, "LLM error: {}", err),
        }
    }
}

impl std::error::Error for LlmError {}

impl From<Box<dyn std::error::Error>> for LlmError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        Self::Other(err)
    }
}

impl From<std::io::Error> for LlmError {
    fn from(err: std::io::Error) -> Self {
        Self::Other(Box::new(err))
    }
}