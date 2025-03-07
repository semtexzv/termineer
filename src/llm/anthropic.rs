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

/// Get the token limit for an Anthropic model
///
/// Uses a pattern-matching approach to determine the appropriate token limit
/// for a given model name, based on the model family and version.
///
/// This function supports all Anthropic Claude models with correct token limits
/// and can be extended as new models are released. It uses a pattern-based approach
/// rather than an explicit list to be more maintainable and future-proof.
///
/// Token limits are sourced from Anthropic's official documentation:
/// https://docs.anthropic.com/claude/docs/model-comparison
fn get_model_token_limit(model_name: &str) -> usize {
    // Default to a conservative limit if no pattern matches
    const DEFAULT_TOKEN_LIMIT: usize = 100_000;

    // Claude 3 and newer models (generally have 200K token context)
    // This covers all Claude 3 variants including:
    // - claude-3-opus-20240229
    // - claude-3-sonnet-20240229
    // - claude-3-haiku-20240307
    // - claude-3-5-sonnet-20240620
    // - claude-3-7-sonnet-20250219
    if model_name.starts_with("claude-3")
        || model_name.starts_with("claude-3.")
        || model_name.contains("claude-3-")
        || model_name.contains("claude-3.5")
        || model_name.contains("claude-3.7")
        || model_name.contains("claude-3-5")
        || model_name.contains("claude-3-7")
    {
        return 200_000;
    }

    // Claude 2.1 (200K token context)
    if model_name.contains("claude-2.1") {
        return 200_000;
    }

    // Claude 2.0 and Claude 2 base (100K token context)
    if model_name.contains("claude-2.0") || model_name.starts_with("claude-2") {
        return 100_000;
    }

    // Claude Instant models (100K token context)
    if model_name.contains("claude-instant") {
        return 100_000;
    }

    // Fall back to default for any unknown models
    // Using a conservative default ensures we don't exceed context windows
    DEFAULT_TOKEN_LIMIT
}

// URLs and version info for the Anthropic API - using lazy initialization for protection
use lazy_static::lazy_static;

lazy_static! {
    static ref API_URL: String =
        obfstr::obfstring!("https://api.anthropic.com/v1/messages").to_string();
    static ref COUNT_TOKENS_URL: String =
        obfstr::obfstring!("https://api.anthropic.com/v1/messages/count_tokens").to_string();
    static ref ANTHROPIC_VERSION: String = obfstr::obfstring!("2023-06-01").to_string();
}

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
    #[allow(dead_code)]
    id: String,
    content: Vec<Content>,
    #[allow(dead_code)]
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

    /// Send a request to the Anthropic API with improved timeout and retry logic
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        // Retry configuration
        let mut attempts = 0;
        let max_attempts = 5; // Increased for better reliability
        let base_retry_delay_ms = 1000; // 1 second initial retry delay
        let max_retry_delay_ms = 30000; // Maximum 30 second retry delay as per TODO
        let request_timeout = Duration::from_secs(180); // 3 minutes timeout (longer as per TODO)

        loop {
            // Log the retry attempt if not the first attempt
            if attempts > 0 {
                bprintln!(dev: "🔄 Retry attempt {} of {} for LLM API call", 
                          attempts, max_attempts);
            }

            // Build the request with timeout
            let request = self
                .client
                .post(&*API_URL)
                .timeout(request_timeout)
                .header("Content-Type", "application/json")
                .header("X-Api-Key", &self.api_key)
                .header("anthropic-version", &*ANTHROPIC_VERSION)
                .header("anthropic-beta", "output-128k-2025-02-19")
                .json(&request_json);
            
            // Send the request
            let response = request.send().await;

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
                            return Err(LlmError::RateLimitError { 
                                retry_after: None 
                            });
                        }

                        // Try to get retry-after header, default to linear backoff if not present
                        let retry_after = match res
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok())
                        {
                            Some(value) => {
                                let delay_ms = value * 1000;
                                bprintln!("⏱️ Rate limit exceeded. Server requested retry after {} seconds", value);
                                delay_ms
                            },
                            None => {
                                // Linear backoff with multiplier
                                let linear_delay = base_retry_delay_ms * (attempts as u64);
                                // Cap at max delay
                                let capped_delay = linear_delay.min(max_retry_delay_ms);
                                bprintln!("⏱️ Rate limit exceeded. Retrying in {} seconds", capped_delay / 1000);
                                capped_delay
                            }
                        };

                        // Sleep for the retry duration
                        sleep(Duration::from_millis(retry_after)).await;
                        continue;
                    } else if res.status().is_server_error() {
                        // Handle server errors (500, 502, 503, etc.)
                        attempts += 1;
                        if attempts >= max_attempts {
                            let status = res.status();
                            let error_text = res
                                .text()
                                .await
                                .unwrap_or_else(|_| "Unknown server error".to_string());
                                
                            return Err(LlmError::ApiError(format!(
                                "Max retries reached. Server error {}: {}",
                                status, error_text
                            )));
                        }
                        
                        // Linear backoff for server errors
                        let linear_delay = base_retry_delay_ms * (attempts as u64);
                        let capped_delay = linear_delay.min(max_retry_delay_ms);
                        
                        bprintln!("⚠️ LLM API server error {}. Retrying in {} seconds (attempt {}/{})", 
                                 res.status(), capped_delay / 1000, attempts, max_attempts);
                        
                        sleep(Duration::from_millis(capped_delay)).await;
                        continue;
                    } else {
                        // Other HTTP errors (4xx client errors except 429)
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
                    // Network-related errors (timeouts, connection issues)
                    attempts += 1;
                    
                    if attempts >= max_attempts {
                        return Err(LlmError::ApiError(format!(
                            "Max retries reached. Network error: {}", 
                            err
                        )));
                    }
                    
                    // Check if it's a timeout error
                    let is_timeout = err.is_timeout();
                    
                    // Linear backoff
                    let linear_delay = base_retry_delay_ms * (attempts as u64);
                    let capped_delay = linear_delay.min(max_retry_delay_ms);
                    
                    if is_timeout {
                        bprintln!("⏱️ LLM API request timed out. Retrying in {} seconds (attempt {}/{})",
                                 capped_delay / 1000, attempts, max_attempts);
                    } else {
                        bprintln!("🌐 Network error: {}. Retrying in {} seconds (attempt {}/{})",
                                 err, capped_delay / 1000, attempts, max_attempts);
                    }
                    
                    sleep(Duration::from_millis(capped_delay)).await;
                    continue;
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

    fn max_token_limit(&self) -> usize {
        // Get the token limit based on the model name pattern
        get_model_token_limit(&self.model)
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
        let mut json = serde_json::to_value(request.clone()).map_err(|e| {
            LlmError::ApiError(format!("Failed to serialize token count request: {}", e))
        })?;

        // Remove info field which is not part of the API schema
        jsonpath::remove(&mut json, "/messages[..]/info").map_err(|e| {
            LlmError::ApiError(format!("Failed to process token count request: {}", e))
        })?;

        // Reuse the same improved timeout and retry logic for token counting
        // by calling send_api_request with the token counting URL
        let timeout = Duration::from_secs(60); // Shorter timeout for token counting
        let mut attempts = 0;
        let max_attempts = 3; // Fewer retries for token counting as it's less critical
        let base_retry_delay_ms = 1000;
        let max_retry_delay_ms = 10000; // 10 seconds max retry for token counting
        
        while attempts < max_attempts {
            if attempts > 0 {
                // Only log retries after the first attempt
                bprintln!(dev: "🔄 Retry attempt {} of {} for token counting", 
                          attempts, max_attempts);
                
                // Linear backoff
                let linear_delay = base_retry_delay_ms * (attempts as u64);
                let capped_delay = linear_delay.min(max_retry_delay_ms);
                sleep(Duration::from_millis(capped_delay)).await;
            }
            
            let result = self
                .client
                .post(&*COUNT_TOKENS_URL)
                .timeout(timeout)
                .header("Content-Type", "application/json")
                .header("X-Api-Key", &self.api_key)
                .header("anthropic-version", &*ANTHROPIC_VERSION)
                .json(&json)
                .send()
                .await;
                
            match result {
                Ok(res) => {
                    if res.status().is_success() {
                        let response: CountTokensResponse = res.json().await.map_err(|e| {
                            LlmError::ApiError(format!("Failed to parse token count response: {}", e))
                        })?;
                        
                        // Create TokenUsage from the response
                        // Note: count_tokens only provides input tokens, output tokens will be 0
                        return Ok(TokenUsage {
                            input_tokens: response.input_tokens,
                            output_tokens: 0, // No output for token counting
                            cache_creation_input_tokens: 0,
                            cache_read_input_tokens: 0,
                        });
                    } else if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS || 
                              res.status().is_server_error() {
                        // Handle rate limiting or server error - retry
                        attempts += 1;
                        continue;
                    } else {
                        // Other errors - don't retry
                        return Err(LlmError::ApiError(format!(
                            "Token count HTTP error {}: {}", 
                            res.status(), 
                            res.text().await.unwrap_or_else(|_| "Unknown error".to_string())
                        )));
                    }
                },
                Err(e) => {
                    // Network error - retry
                    attempts += 1;
                    if attempts >= max_attempts {
                        return Err(LlmError::ApiError(format!("Token count request failed after {} attempts: {}", max_attempts, e)));
                    }
                }
            }
        }
        
        // If we've exhausted all retries
        Err(LlmError::ApiError("Max retries reached for token counting".to_string()))
    }
}
