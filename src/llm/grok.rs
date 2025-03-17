//! xAI Grok API Provider
//!
//! Implementation of the LLM provider for xAI's Grok language models.

use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeSet;

// API base URL for Grok
const API_BASE_URL: &str = "https://api.x.ai/v1";

/// Get the token limit for a Grok model
fn get_model_token_limit(model_name: &str) -> usize {
    const DEFAULT_TOKEN_LIMIT: usize = 32_000;

    // Model-specific token limits
    if model_name.contains("grok-2") {
        return 128_000; // 128K context window for Grok 2
    }

    if model_name.contains("grok-beta") {
        return 32_000; // Older model has smaller context window
    }

    DEFAULT_TOKEN_LIMIT
}

/// Grok API request for chat completions
#[derive(Debug, Serialize)]
struct GrokChatRequest {
    model: String,
    messages: Vec<GrokMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
}

/// Grok chat message format
#[derive(Debug, Serialize, Deserialize)]
struct GrokMessage {
    role: String,
    content: String,
}

/// Grok API response structure
#[derive(Debug, Deserialize)]
struct GrokResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<GrokChoice>,
    usage: GrokUsage,
}

#[derive(Debug, Deserialize)]
struct GrokChoice {
    index: u32,
    message: GrokMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrokUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// Implementation of LLM provider for Grok
pub struct GrokBackend {
    /// API key for Grok
    api_key: String,

    /// Model name to use
    model: String,

    /// HTTP client
    client: reqwest::Client,
}

impl GrokBackend {
    /// Create a new Grok provider with the specified API key and model
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Convert Termineer messages to Grok format
    fn convert_messages(&self, messages: &[Message], system: Option<&str>) -> Vec<GrokMessage> {
        let mut grok_messages = Vec::new();

        // Add system message if provided
        if let Some(sys) = system {
            grok_messages.push(GrokMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }

        // Convert regular messages
        for message in messages {
            // Extract text content
            let text = match &message.content {
                Content::Text { text } => text.clone(),
                _ => continue, // Skip non-text content for now
            };

            // Map roles to Grok format (similar to OpenAI)
            let role = match message.role.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                "system" => "system",
                _ => continue, // Skip unknown roles
            };

            grok_messages.push(GrokMessage {
                role: role.to_string(),
                content: text,
            });
        }

        grok_messages
    }

    /// Send a request to the Grok API using the standardized retry utility
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        use crate::llm::retry_utils::{send_api_request_with_retry, RetryConfig};

        // Use standard retry configuration
        const MAX_ATTEMPTS: u32 = 5;
        const BASE_RETRY_DELAY_MS: u64 = 1000;
        const MAX_RETRY_DELAY_MS: u64 = 30000;
        const REQUEST_TIMEOUT_SECS: u64 = 180;

        let config = RetryConfig {
            max_attempts: MAX_ATTEMPTS,
            base_delay_ms: BASE_RETRY_DELAY_MS,
            max_delay_ms: MAX_RETRY_DELAY_MS,
            timeout_secs: REQUEST_TIMEOUT_SECS,
            use_exponential: false,
        };

        // Construct the API URL
        let url = format!("{}{}", API_BASE_URL, path);

        // Create request with proper headers
        let prepare_request = || {
            self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request_json)
        };

        // Use the standardized retry utility
        send_api_request_with_retry::<T, _>(&self.client, &url, prepare_request, config, "Grok")
            .await
    }
}

#[async_trait]
impl Backend for GrokBackend {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Grok doesn't support thinking or cache points
        if thinking_budget.is_some() {
            crate::bprintln!(dev: "Thinking is not supported by Grok models, ignoring thinking_budget");
        }

        if cache_points.is_some() {
            crate::bprintln!(dev: "Cache points are not supported by Grok models, ignoring cache_points");
        }

        // Convert messages to Grok format
        let grok_messages = self.convert_messages(messages, system);

        // Default max tokens if not provided
        let default_max_tokens = 2048;
        let tokens = max_tokens.unwrap_or(default_max_tokens);

        // Prepare stop sequences
        let stop_seqs = stop_sequences.map(|seqs| seqs.to_vec()).unwrap_or_default();

        // Create request
        let request = GrokChatRequest {
            model: self.model.clone(),
            messages: grok_messages,
            max_tokens: Some(tokens as u32),
            temperature: Some(0.7), // Default temperature
            stop: stop_seqs,
        };

        // Send request to the Grok chat endpoint
        let grok_response: GrokResponse = self
            .send_api_request("/chat/completions", serde_json::to_value(request).unwrap())
            .await?;

        // Extract response text from the first choice
        if grok_response.choices.is_empty() {
            return Err(LlmError::ApiError(
                "No response choices returned from Grok API".to_string(),
            ));
        }

        let choice = &grok_response.choices[0];
        let response_text = choice.message.content.clone();

        // Create token usage info
        let token_usage = TokenUsage {
            input_tokens: grok_response.usage.prompt_tokens as usize,
            output_tokens: grok_response.usage.completion_tokens as usize,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        Ok(LlmResponse {
            content: vec![Content::Text {
                text: response_text,
            }],
            usage: Some(token_usage),
            stop_reason: choice.finish_reason.clone(),
            stop_sequence: None,
        })
    }

    async fn count_tokens(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Result<TokenUsage, LlmError> {
        // Grok doesn't have a tokenization endpoint, so we'll use a character-based estimate
        // Rough approximation: 1 token ≈ 4 characters

        let char_count = messages
            .iter()
            .filter_map(|msg| {
                if let Content::Text { text } = &msg.content {
                    Some(text.len())
                } else {
                    None
                }
            })
            .sum::<usize>();

        // Add system message if present
        let total_chars = if let Some(sys) = system {
            char_count + sys.len()
        } else {
            char_count
        };

        // Estimate tokens (approximate)
        let estimated_tokens = (total_chars as f64 / 4.0).ceil() as usize;

        Ok(TokenUsage {
            input_tokens: estimated_tokens,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        })
    }

    fn max_token_limit(&self) -> usize {
        get_model_token_limit(&self.model)
    }

    fn name(&self) -> &str {
        "grok"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
