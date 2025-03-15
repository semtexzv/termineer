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
    if model_name.starts_with("claude-3") {
        return 200_000;
    }

    // Claude 2.1 (200K token context)
    if model_name.contains("claude-2.1") {
        return 200_000;
    }

    // Claude 2.0 and Claude 2 base (100K token context)
    if model_name.contains("claude-2") |  model_name.contains("claude-instant") {
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

    // Centralized retry configuration constants
    const MAX_ATTEMPTS: u32 = 5; // Maximum number of retry attempts
    const BASE_RETRY_DELAY_MS: u64 = 1000; // Base delay for first retry (1 second)
    const MAX_RETRY_DELAY_MS: u64 = 30000; // Maximum retry delay (30 seconds) as per TODO
    const REQUEST_TIMEOUT_SECS: u64 = 180; // 3 minutes timeout (within 100-200s range from TODO)
    const TOKEN_COUNT_TIMEOUT_SECS: u64 = 60; // 1 minute timeout for token counting
    
    /// Calculate exponential backoff delay with jitter
    fn calculate_backoff_delay(attempt: u32) -> u64 {
        if attempt == 0 {
            return 0; // No delay on first attempt
        }
        
        // Exponential backoff: delay = base * 2^(attempt-1)
        let exponent = attempt.saturating_sub(1) as u32;
        let exponential_delay = Self::BASE_RETRY_DELAY_MS * (2_u64.saturating_pow(exponent));
        
        // Add jitter (±10%) to prevent thundering herd problem
        let jitter_range = exponential_delay / 10; // 10% of delay
        let jitter = rand::random::<u64>() % (jitter_range * 2);
        let with_jitter = exponential_delay.saturating_add(jitter).saturating_sub(jitter_range);
        
        // Cap at maximum delay
        with_jitter.min(Self::MAX_RETRY_DELAY_MS)
    }
    
    /// Send a request to the Anthropic API with improved timeout and retry logic
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        request_json: serde_json::Value,
        url: &str,
        timeout: Duration,
    ) -> Result<T, LlmError> {
        let mut attempts = 0;

        loop {
            // Log the retry attempt if not the first attempt
            if attempts > 0 {
                bprintln!(dev: "🔄 Retry attempt {} of {} for LLM API call", 
                          attempts, Self::MAX_ATTEMPTS);
            }

            // Build the request with timeout
            let request = self
                .client
                .post(url)
                .timeout(timeout)
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
                        if attempts >= Self::MAX_ATTEMPTS {
                            return Err(LlmError::RateLimitError { 
                                retry_after: None 
                            });
                        }

                        // Try to get retry-after header, default to backoff strategy if not present
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
                                // Use exponential backoff with jitter
                                let delay_ms = Self::calculate_backoff_delay(attempts);
                                bprintln!("⏱️ Rate limit exceeded. Retrying in {} seconds", delay_ms / 1000);
                                delay_ms
                            }
                        };

                        // Sleep for the retry duration
                        sleep(Duration::from_millis(retry_after)).await;
                        continue;
                    } else if res.status().is_server_error() {
                        // Handle server errors (500, 502, 503, etc.)
                        attempts += 1;
                        if attempts >= Self::MAX_ATTEMPTS {
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
                        
                        // Exponential backoff for server errors
                        let delay_ms = Self::calculate_backoff_delay(attempts);
                        
                        bprintln!("⚠️ LLM API server error {}. Retrying in {} seconds (attempt {}/{})", 
                                 res.status(), delay_ms / 1000, attempts, Self::MAX_ATTEMPTS);
                        
                        sleep(Duration::from_millis(delay_ms)).await;
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
                    
                    if attempts >= Self::MAX_ATTEMPTS {
                        if err.is_timeout() {
                            return Err(LlmError::ApiError(format!(
                                "Request timed out after {} seconds and {} retry attempts", 
                                timeout.as_secs(),
                                Self::MAX_ATTEMPTS
                            )));
                        } else {
                            return Err(LlmError::ApiError(format!(
                                "Max retries reached. Network error: {}", 
                                err
                            )));
                        }
                    }
                    
                    // Check if it's a timeout error
                    let is_timeout = err.is_timeout();
                    
                    // Exponential backoff with jitter
                    let delay_ms = Self::calculate_backoff_delay(attempts);
                    
                    if is_timeout {
                        bprintln!("⏱️ LLM API request timed out after {} seconds. Retrying in {} seconds (attempt {}/{})",
                                 timeout.as_secs(), delay_ms / 1000, attempts, Self::MAX_ATTEMPTS);
                    } else {
                        bprintln!("🌐 Network error: {}. Retrying in {} seconds (attempt {}/{})",
                                 err, delay_ms / 1000, attempts, Self::MAX_ATTEMPTS);
                    }
                    
                    sleep(Duration::from_millis(delay_ms)).await;
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
            thinking: thinking_budget.and_then(|budget| {
                if budget > 0 {
                    Some(ThinkingConfig {
                        budget_tokens: budget,
                        type_: ThinkingType::Enabled,
                    })
                } else {
                    None // Disable thinking when budget is 0
                }
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

        // Send the request with appropriate URL and timeout
        let response: MessageResponse = self.send_api_request(
            json,
            &*API_URL,
            Duration::from_secs(Self::REQUEST_TIMEOUT_SECS)
        ).await?;

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

        // Use the improved send_api_request method with appropriate URL and timeout
        // This reuses the same robust retry/timeout logic we implemented earlier
        let response: CountTokensResponse = self.send_api_request(
            json,
            &*COUNT_TOKENS_URL,
            Duration::from_secs(Self::TOKEN_COUNT_TIMEOUT_SECS)
        ).await?;
        
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
