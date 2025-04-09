//! Cohere API Provider
//!
//! Implementation of the LLM provider for Cohere's language models.

use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeSet;

// API base URL for Cohere
const API_BASE_URL: &str = "https://api.cohere.ai/v1";

/// Get the token limit for a Cohere model
///
/// Uses a pattern-matching approach to determine the appropriate token limit
/// for a given model name, based on the model family and version.
fn get_model_token_limit(model_name: &str) -> usize {
    // Default to a conservative limit if no pattern matches
    const DEFAULT_TOKEN_LIMIT: usize = 8_000;

    // Command models and their token limits
    if model_name.contains("command-r") || model_name == "command-r" {
        return 128_000; // 128K tokens for Command-R
    }
    if model_name.contains("command-r-plus") || model_name == "command-r-plus" {
        return 128_000; // 128K tokens for Command-R+
    }
    if model_name.contains("command-light") || model_name == "command-light" {
        return 4_000; // 4K tokens for Command Light
    }

    // Legacy command model
    if model_name.contains("command") || model_name == "command" {
        return 4_096; // 4K tokens for Command
    }

    // Fall back to default for any unknown models
    DEFAULT_TOKEN_LIMIT
}

/// Cohere API request type for chat
#[derive(Debug, Serialize)]
struct CohereRequest {
    model: String,
    message: String,
    chat_history: Vec<CohereMessage>,
    preamble: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop_sequences: Vec<String>,
}

/// Cohere chat history message
#[derive(Debug, Serialize)]
struct CohereMessage {
    role: String,
    message: String,
}

/// Cohere API response for chat
#[derive(Debug, Deserialize)]
struct CohereResponse {
    text: String,
    #[allow(dead_code)]
    generation_id: String,
    finish_reason: Option<String>,
    token_count: Option<CohereTokenCount>,
}

/// Cohere token count information
#[derive(Debug, Deserialize)]
struct CohereTokenCount {
    prompt_tokens: u32,
    response_tokens: u32,
    #[allow(dead_code)]
    total_tokens: u32,
}

/// Cohere token count API request
#[derive(Debug, Serialize)]
struct CohereTokenCountRequest {
    texts: Vec<String>,
}

/// Cohere token count API response
#[derive(Debug, Deserialize)]
struct CohereTokenCountResponse {
    tokens: Vec<u32>,
}

/// Implementation of LLM provider for Cohere
pub struct CohereBackend {
    /// API key for Cohere
    api_key: String,

    /// Model name to use
    model: String,

    /// HTTP client
    client: reqwest::Client,
}

impl CohereBackend {
    /// Create a new Cohere provider with the specified API key and model
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Convert Termineer messages to Cohere format
    fn convert_messages(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> (Vec<CohereMessage>, String, Option<String>) {
        let mut chat_history = Vec::new();
        let mut preamble = None;
        let mut current_message = String::new();

        // Process messages
        for (i, message) in messages.iter().enumerate() {
            // Handle system messages
            if message.role == "system" {
                if let Content::Text { text } = &message.content {
                    preamble = Some(text.clone());
                }
                continue;
            }

            // Extract text content
            let text = match &message.content {
                Content::Text { text } => text.clone(),
                _ => continue, // Skip non-text content for now
            };

            // If the message is from user and it's the last one, it becomes the current message
            if message.role == "user" && i == messages.len() - 1 {
                current_message = text;
                continue;
            }

            // Map roles to Cohere format
            let role = match message.role.as_str() {
                "user" => "USER",
                "assistant" => "CHATBOT",
                _ => continue, // Skip unknown roles
            };

            // Add to chat history
            chat_history.push(CohereMessage {
                role: role.to_string(),
                message: text,
            });
        }

        // If no current message was set (no user message at the end),
        // use an empty string or a default placeholder
        if current_message.is_empty() && !chat_history.is_empty() {
            current_message = "Continue the conversation".to_string();
        }

        // Override preamble with system parameter if provided
        if let Some(sys) = system {
            preamble = Some(sys.to_string());
        }

        (chat_history, current_message, preamble)
    }

    /// Send a request to the Cohere API using the standardized retry utility
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        use crate::llm::retry_utils::{send_api_request_with_retry, RetryConfig};

        // Retry configuration constants - keep the same values for consistency
        const MAX_ATTEMPTS: u32 = 5;
        const BASE_RETRY_DELAY_MS: u64 = 1000;
        const MAX_RETRY_DELAY_MS: u64 = 30000;
        const REQUEST_TIMEOUT_SECS: u64 = 180;

        // Create retry configuration
        let config = RetryConfig {
            max_attempts: MAX_ATTEMPTS,
            base_delay_ms: BASE_RETRY_DELAY_MS,
            max_delay_ms: MAX_RETRY_DELAY_MS,
            timeout_secs: REQUEST_TIMEOUT_SECS,
            use_exponential: false, // Use linear backoff as specified in TODO
        };

        // Construct the API URL
        let url = format!("{}{}", API_BASE_URL, path);

        // Create a request builder closure that includes all necessary headers
        let prepare_request = || {
            self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request_json)
        };

        // Use the standardized retry utility
        send_api_request_with_retry::<T, _>(&self.client, &url, prepare_request, config, "Cohere")
            .await
    }
}

#[async_trait]
impl Backend for CohereBackend {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Cohere doesn't support thinking or cache points
        if thinking_budget.is_some() {
            // bprintln!(dev: "Thinking is not supported by Cohere models, ignoring thinking_budget");
        }

        if cache_points.is_some() {
            // bprintln!(dev: "Cache points are not supported by Cohere models, ignoring cache_points");
        }

        // Convert messages to Cohere format
        let (chat_history, current_message, preamble) = self.convert_messages(messages, system);

        // Default max tokens if not provided
        let default_max_tokens = 2048; // Reasonable default
        let tokens = max_tokens.unwrap_or(default_max_tokens);

        // Prepare stop sequences
        let stop_seqs = stop_sequences.map(|seqs| seqs.to_vec()).unwrap_or_default();

        // Create request
        let request = CohereRequest {
            model: self.model.clone(),
            message: current_message,
            chat_history,
            preamble,
            max_tokens: Some(tokens as u32),
            temperature: Some(0.8), // Default temperature
            stop_sequences: stop_seqs,
        };

        // Send request to the Cohere chat endpoint
        let cohere_response: CohereResponse = self
            .send_api_request("/chat", serde_json::to_value(request).unwrap())
            .await?;

        // Extract token usage
        let token_usage = if let Some(count) = cohere_response.token_count {
            TokenUsage {
                input_tokens: count.prompt_tokens as usize,
                output_tokens: count.response_tokens as usize,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        } else {
            // If token count is not provided, estimate based on text length
            TokenUsage {
                input_tokens: 0, // Unable to accurately estimate input tokens
                output_tokens: cohere_response.text.len() / 4, // Rough estimate
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        };

        Ok(LlmResponse {
            content: vec![Content::Text {
                text: cohere_response.text,
            }],
            usage: Some(token_usage),
            stop_reason: cohere_response.finish_reason,
            stop_sequence: None, // Not provided by Cohere
        })
    }


    fn max_token_limit(&self) -> usize {
        get_model_token_limit(&self.model)
    }

    fn name(&self) -> &str {
        "cohere"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
