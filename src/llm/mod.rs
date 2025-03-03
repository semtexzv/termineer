//! LLM provider abstraction layer
//!
//! This module defines traits and types for interacting with
//! different LLM providers (Anthropic, Google, etc.)

pub use async_trait::async_trait;

pub mod anthropic;
pub mod factory;
mod types;

pub use self::factory::{create_backend, create_backend_for_task};
pub use self::types::*;
use std::collections::BTreeSet;

/// Common trait for all LLM backends
#[async_trait]
pub trait Backend: Send + Sync {
    /// Send a message to the LLM and get a response
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>, // Maximum tokens to generate in the response
    ) -> Result<LlmResponse, LlmError>;
    
    /// Count tokens for messages without making a full API request
    /// 
    /// This allows efficiently tracking token usage without guessing.
    /// Different LLM providers have different token counting algorithms, so this
    /// method delegates to the provider-specific implementation.
    ///
    /// # Arguments
    /// * `messages` - The conversation messages to count tokens for
    /// * `system` - Optional system prompt
    ///
    /// # Returns
    /// Token usage statistics or an error
    async fn count_tokens(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Result<TokenUsage, LlmError>;
    
    /// Get the maximum token limit for this model
    /// 
    /// Returns the total context window size for the current model,
    /// including both input and output tokens. This is used for
    /// conversation truncation and to prevent exceeding model limits.
    ///
    /// # Returns
    /// Maximum token limit as documented by the model provider
    fn max_token_limit(&self) -> usize;
    
    /// Get the safe input token limit for this model
    /// 
    /// Returns a conservative limit that leaves room for the model's response.
    /// This is typically 80-90% of the max limit, ensuring we don't exceed
    /// the context window when the model generates its response.
    ///
    /// Used by the conversation truncation system to determine when
    /// truncation should be applied.
    ///
    /// # Returns
    /// Safe input token limit (default: 80% of max_token_limit)
    fn safe_input_token_limit(&self) -> usize {
        // Default implementation: 80% of max token limit
        (self.max_token_limit() as f64 * 0.8) as usize
    }

    /// Get the provider name
    /// Included in the API for provider identification but not currently used
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Get the model name
    /// Included in the API for model identification but not currently used
    #[allow(dead_code)]
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
            }
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
