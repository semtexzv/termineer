//! Anthropic Claude API Provider
//!
//! Implementation of the LLM provider for Anthropic's Claude models.

use crate::jsonpath;
use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::sleep;

// Constants for the Anthropic API
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const COUNT_TOKENS_URL: &str = "https://api.anthropic.com/v1/messages/count_tokens";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic request configuration
#[derive(Serialize, Clone)]
struct ThinkingConfig {
    budget_tokens: usize,
    #[serde(rename = "type")]
    type_: ThinkingType,
}

/// Type of thinking to enable
#[derive(Serialize, Clone)]
enum ThinkingType {
    #[serde(rename = "enabled")]
    Enabled,
}

/// Message request to the Anthropic API
#[derive(Serialize, Clone)]
struct MessageRequest {
    model: String,
    max_tokens: usize,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

/// Response from the Anthropic API
#[derive(Deserialize, Debug)]
struct MessageResponse {
    id: String,
    content: Vec<Content>,
    model: String,
    usage: Option<TokenUsage>,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

/// Request to count tokens
#[derive(Serialize, Clone)]
struct CountTokensRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

/// Response from the token counting endpoint
#[derive(Deserialize, Debug)]
struct CountTokensResponse {
    input_tokens: usize,
}

/// Implementation of LLM provider for Anthropic
pub struct Anthropic {
    /// API key for Anthropic
    api_key: String,

    /// Model name to use
    model: String,

    /// HTTP client
    client: reqwest::Client,
}

impl Anthropic {
    /// Create a new Anthropic provider with the specified API key and model
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Send a request to the Anthropic API with retry logic
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        // Retry configuration
        let mut attempts = 0;
        let max_attempts = 3;
        let base_retry_delay_ms = 1000; // 1 second initial retry delay

        loop {
            let response = self
                .client
                .post(API_URL)
                .header("Content-Type", "application/json")
                .header("X-Api-Key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("anthropic-beta", "output-128k-2025-02-19")
                .json(&request_json)
                .send()
                .await;

            match response {
                Ok(res) => {
                    if res.status().is_success() {
                        return res.json::<T>().await.map_err(|e| {
                            LlmError::ApiError(format!("Failed to parse response: {}", e))
                        });
                    } else if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        // Handle rate limiting (429 Too Many Requests)
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(LlmError::ApiError(format!(
                                "Max retry attempts reached for rate limiting: {}",
                                res.status()
                            )));
                        }

                        // Try to get retry-after header, default to exponential backoff if not present
                        let retry_after = match res
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                        {
                            Some(value) => value * 1000,
                            None => {
                                // Exponential backoff with jitter
                                let exponential_delay =
                                    base_retry_delay_ms * 2u64.pow(attempts as u32);
                                exponential_delay
                            }
                        };

                        // Sleep for the retry duration
                        sleep(Duration::from_millis(retry_after)).await;
                        continue;
                    } else {
                        let status = res.status();
                        let error_text = res
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown error".to_string());

                        return Err(LlmError::ApiError(format!(
                            "HTTP error {}: {}",
                            status, error_text
                        )));
                    }
                }
                Err(err) => {
                    return Err(LlmError::ApiError(format!("HTTP request error: {}", err)));
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl Backend for Anthropic {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Default max tokens if not provided
        let default_max_tokens = 32768; // Large default for Claude's capabilities
        let tokens = max_tokens.unwrap_or(default_max_tokens);

        // Create the message request
        let request = MessageRequest {
            model: self.model.clone(),
            max_tokens: tokens,
            messages: messages.to_vec(),
            system: system.map(|s| s.to_string()),
            stop_sequences: stop_sequences.map(|s| s.to_vec()),
            thinking: thinking_budget.map(|budget| ThinkingConfig {
                budget_tokens: budget,
                type_: ThinkingType::Enabled,
            }),
        };

        // Convert to JSON and prepare for the API
        let mut json = serde_json::to_value(request.clone())
            .map_err(|e| LlmError::ApiError(format!("Failed to serialize request: {}", e)))?;

        // Remove info field which is not part of the API schema
        jsonpath::remove(&mut json, "/messages[..]/info")
            .map_err(|e| LlmError::ApiError(format!("Failed to process request: {}", e)))?;

        // Add cache annotation to cached conversation points
        for point in cache_points.iter().flat_map(|v| v.iter()) {
            let path = format!("/messages[{}]/content[-1]/cache_control", point);
            jsonpath::insert(&mut json, &path, json!({"type": "ephemeral"}))
                .map_err(|e| LlmError::ApiError(format!("Failed to process request: {}", e)))?;
        }

        // Send the request
        let response: MessageResponse = self.send_api_request(json).await?;

        // Convert to LlmResponse with stop information
        Ok(LlmResponse {
            content: response.content,
            usage: response.usage,
            stop_reason: response.stop_reason,
            stop_sequence: response.stop_sequence,
        })
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    fn model(&self) -> &str {
        &self.model
    }
    
    async fn count_tokens(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Result<TokenUsage, LlmError> {
        // Create the token counting request
        let request = CountTokensRequest {
            model: self.model.clone(),
            messages: messages.to_vec(),
            system: system.map(|s| s.to_string()),
        };

        // Convert to JSON and prepare for the API
        let mut json = serde_json::to_value(request.clone())
            .map_err(|e| LlmError::ApiError(format!("Failed to serialize token count request: {}", e)))?;

        // Remove info field which is not part of the API schema
        jsonpath::remove(&mut json, "/messages[..]/info")
            .map_err(|e| LlmError::ApiError(format!("Failed to process token count request: {}", e)))?;

        // Send the request to the count_tokens endpoint
        let client_response = self
            .client
            .post(COUNT_TOKENS_URL)
            .header("Content-Type", "application/json")
            .header("X-Api-Key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&json)
            .send()
            .await
            .map_err(|e| LlmError::ApiError(format!("Token count request failed: {}", e)))?;
            
        let response: CountTokensResponse = client_response
            .json()
            .await
            .map_err(|e| LlmError::ApiError(format!("Failed to parse token count response: {}", e)))?;

        // Create TokenUsage from the response
        // Note: count_tokens only provides input tokens, output tokens will be 0
        Ok(TokenUsage {
            input_tokens: response.input_tokens,
            output_tokens: 0, // No output for token counting
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        })
    }
}
