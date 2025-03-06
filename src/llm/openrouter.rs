//! OpenRouter API Provider
//!
//! Implementation of the LLM provider for OpenRouter.
//! OpenRouter provides access to multiple LLM models through a unified API.
#![allow(static_mut_refs)]
use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use bprintln;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use tokio::time::sleep;

// URLs for the OpenRouter API - using lazy initialization for protection
use lazy_static::lazy_static;

lazy_static! {
    static ref API_URL: String =
        obfstr::obfstring!("https://openrouter.ai/api/v1/chat/completions").to_string();
    static ref MODELS_API_URL: String =
        obfstr::obfstring!("https://openrouter.ai/api/v1/models").to_string();
}

// Global cache for model information - lazily initialized
static MODEL_INFO_INIT: Once = Once::new();
static mut MODEL_INFO_CACHE: Option<Arc<Mutex<Option<Vec<ModelInfo>>>>> = None;

/// Model pricing information
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelPricing {
    /// Cost per token for prompt (input)
    #[serde(with = "crate::serde_utils::string_or_number")]
    pub prompt: f64,
    /// Cost per token for completion (output)
    #[serde(with = "crate::serde_utils::string_or_number")]
    pub completion: f64,
}

// The deserialize_string_or_number function is now imported from serde_utils::string_or_number

/// Model information from OpenRouter API
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelInfo {
    /// Model ID to use in API calls
    pub id: String,
    /// Human-readable model name
    pub name: String,
    /// Model description
    pub description: String,
    /// Pricing information
    pub pricing: ModelPricing,
}

/// Response from OpenRouter models endpoint
#[derive(Debug, Deserialize, Serialize)]
pub struct ModelListResponse {
    /// List of available models
    pub data: Vec<ModelInfo>,
}

/// Determine the token limit for different models available through OpenRouter
///
/// OpenRouter provides access to many different models with varying token limits.
/// This function maps models to their respective token limits based on documentation.
fn get_model_token_limit(model_name: &str) -> usize {
    // Default to a conservative limit if no pattern matches
    const DEFAULT_TOKEN_LIMIT: usize = 8192;

    // OpenAI models
    if model_name.contains("gpt-4o") {
        return 128_000;
    } else if model_name.contains("gpt-4-turbo") {
        return 128_000;
    } else if model_name.contains("gpt-4") {
        return 8_192;
    } else if model_name.contains("gpt-3.5-turbo") {
        return 16_384;
    }
    // Anthropic models via OpenRouter
    else if model_name.contains("claude-3-opus") {
        return 200_000;
    } else if model_name.contains("claude-3-sonnet") {
        return 200_000;
    } else if model_name.contains("claude-3-haiku") {
        return 200_000;
    } else if model_name.contains("claude-instant") {
        return 100_000;
    }

    // Other models - use a conservative default
    DEFAULT_TOKEN_LIMIT
}

/// OpenRouter request body
#[derive(Serialize, Debug, Clone)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// OpenRouter message format
#[derive(Serialize, Deserialize, Debug, Clone)]
struct OpenRouterMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// OpenRouter response format
#[derive(Deserialize, Debug)]
struct OpenRouterResponse {
    #[serde(default)]
    #[allow(dead_code)]
    id: String,
    choices: Vec<OpenRouterChoice>,
    #[allow(dead_code)]
    model: String,
    usage: Option<OpenRouterUsage>,
}

/// Choice in OpenRouter response
#[derive(Deserialize, Debug)]
struct OpenRouterChoice {
    #[allow(dead_code)]
    index: usize,
    message: OpenRouterMessage,
    finish_reason: Option<String>,
}

/// Token usage information
#[derive(Deserialize, Debug)]
struct OpenRouterUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[allow(dead_code)]
    total_tokens: usize,
}

/// Implementation of LLM provider for OpenRouter
pub struct OpenRouter {
    /// API key for OpenRouter
    api_key: String,

    /// Model name to use
    model: String,

    /// HTTP client
    client: reqwest::Client,

    /// Optional site URL for OpenRouter headers
    site_url: Option<String>,

    /// Optional site name for OpenRouter headers
    site_name: Option<String>,
}

impl OpenRouter {
    /// Create a new OpenRouter provider with the specified API key and model
    pub fn new(api_key: String, model: String) -> Self {
        // Initialize the global model cache if needed
        Self::init_model_cache();

        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
            site_url: None,
            site_name: None,
        }
    }

    /// Initialize the global model cache
    fn init_model_cache() {
        MODEL_INFO_INIT.call_once(|| unsafe {
            MODEL_INFO_CACHE = Some(Arc::new(Mutex::new(None)));
        });
    }

    /// Get the global model cache
    fn get_model_cache() -> Arc<Mutex<Option<Vec<ModelInfo>>>> {
        unsafe { MODEL_INFO_CACHE.as_ref().unwrap().clone() }
    }

    /// Set the site URL and name for OpenRouter headers
    pub fn with_site_info(mut self, site_url: Option<String>, site_name: Option<String>) -> Self {
        self.site_url = site_url;
        self.site_name = site_name;
        self
    }

    /// List all available models from OpenRouter
    pub async fn list_available_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        // Check cache first
        let cache = Self::get_model_cache();

        {
            let cache_guard = cache.lock().unwrap();
            if let Some(models) = &*cache_guard {
                return Ok(models.clone());
            }
        }

        // Fetch models from API if not in cache
        let client = reqwest::Client::new();
        let response = client
            .get(&*MODELS_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| LlmError::ApiError(format!("Failed to fetch models: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LlmError::ApiError(format!(
                "Failed to fetch models: HTTP {} - {}",
                status, error_text
            )));
        }

        let model_response: ModelListResponse = response
            .json()
            .await
            .map_err(|e| LlmError::ApiError(format!("Failed to parse models response: {}", e)))?;

        // Update cache
        {
            let mut cache_guard = cache.lock().unwrap();
            *cache_guard = Some(model_response.data.clone());
        }

        Ok(model_response.data)
    }

    /// Convert our internal messages to OpenRouter format
    fn convert_messages(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Vec<OpenRouterMessage> {
        let mut openrouter_messages = Vec::new();

        // Add system message if provided
        if let Some(system_content) = system {
            openrouter_messages.push(OpenRouterMessage {
                role: "system".to_string(),
                content: Some(system_content.to_string()),
                name: None,
            });
        }

        // Add conversation messages
        for message in messages {
            // Extract text content from our internal message format
            let content = match &message.content {
                Content::Text { text } => Some(text.clone()),
                _ => None, // Skip non-text content types
            };

            openrouter_messages.push(OpenRouterMessage {
                role: message.role.clone(),
                content,
                name: None,
            });
        }

        openrouter_messages
    }

    /// Send a request to the OpenRouter API with retry logic
    async fn send_api_request(
        &self,
        request: OpenRouterRequest,
    ) -> Result<OpenRouterResponse, LlmError> {
        // Retry configuration
        let mut attempts = 0;
        let max_attempts = 3;
        let base_retry_delay_ms = 1000; // 1 second initial retry delay

        loop {
            let mut request_builder = self
                .client
                .post(&*API_URL)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key));

            // Add optional OpenRouter-specific headers
            if let Some(ref site_url) = self.site_url {
                request_builder = request_builder.header("HTTP-Referer", site_url);
            }

            if let Some(ref site_name) = self.site_name {
                request_builder = request_builder.header("X-Title", site_name);
            }

            let response = request_builder.json(&request).send().await;

            match response {
                Ok(res) => {
                    if res.status().is_success() {
                        // Get the raw response text first
                        let raw_response = match res.text().await {
                            Ok(text) => text,
                            Err(e) => {
                                return Err(LlmError::ApiError(format!(
                                    "Failed to read response body: {}",
                                    e
                                )));
                            }
                        };

                        // Now try to parse the JSON
                        match serde_json::from_str::<OpenRouterResponse>(&raw_response) {
                            Ok(parsed) => return Ok(parsed),
                            Err(e) => {
                                // Log the raw response for debugging
                                bprintln!("OpenRouter API raw response (JSON parse failed):");
                                bprintln!("{}", raw_response);

                                return Err(LlmError::ApiError(format!(
                                    "Failed to parse OpenRouter response as JSON: {}. Raw response has been logged.", e
                                )));
                            }
                        }
                    } else if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        // Handle rate limiting
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(LlmError::ApiError(format!(
                                "Max retry attempts reached for rate limiting: {}",
                                res.status()
                            )));
                        }

                        // Try to get retry-after header
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

    /// Convert OpenRouter response to our internal format
    fn convert_response(&self, response: OpenRouterResponse) -> Result<LlmResponse, LlmError> {
        if response.choices.is_empty() {
            return Err(LlmError::ApiError(
                "Empty response from OpenRouter".to_string(),
            ));
        }

        let choice = &response.choices[0];
        let content_text = choice.message.content.clone().unwrap_or_default();
        let content = vec![Content::Text { text: content_text }];

        // Convert usage information
        let usage = response.usage.map(|u| TokenUsage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });

        Ok(LlmResponse {
            content,
            usage,
            stop_reason: choice.finish_reason.clone(),
            stop_sequence: None, // OpenRouter doesn't provide the stop sequence
        })
    }
}

#[async_trait::async_trait]
impl Backend for OpenRouter {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        _thinking_budget: Option<usize>, // OpenRouter doesn't support thinking
        _cache_points: Option<&BTreeSet<usize>>, // OpenRouter doesn't support caching
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Try to fetch and log available models if this is the first API call
        let cache = Self::get_model_cache();
        let should_fetch_models = {
            let cache_guard = cache.lock().unwrap();
            cache_guard.is_none()
        };

        if should_fetch_models {
            match self.list_available_models().await {
                Ok(models) => {
                    bprintln!("\nAvailable OpenRouter Models:");
                    bprintln!("===========================");
                    for model in &models {
                        bprintln!(
                            "- {} (${:.4}/1K input, ${:.4}/1K output)",
                            model.id,
                            model.pricing.prompt * 1000.0,
                            model.pricing.completion * 1000.0
                        );
                    }
                    bprintln!("===========================\n");
                }
                Err(e) => {
                    bprintln!("Note: Could not fetch OpenRouter models: {}", e);
                }
            }
        }

        // Convert messages to OpenRouter format
        let openrouter_messages = self.convert_messages(messages, system);

        // Log request details (number of messages only, not content)
        // bprintln!("OpenRouter request: model={}, message_count={}, system_prompt={}",
        //          self.model,
        //          openrouter_messages.len(),
        //          system.is_some());

        // Build the request
        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: openrouter_messages,
            max_tokens,
            stop: stop_sequences.map(|s| s.to_vec()),
            stream: Some(false),
            temperature: Some(0.7),
        };

        // Send the request
        let response = self.send_api_request(request).await?;

        // Convert the response to our internal format
        self.convert_response(response)
    }

    fn name(&self) -> &str {
        "openrouter"
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
        // OpenRouter doesn't have a dedicated token counting endpoint
        // For simplicity, we'll just perform a rough estimate
        // A better approach would be to use a tokenizer library

        // Estimate: ~1.3 tokens per word for English text
        let mut total_tokens = 0;

        // Count system prompt
        if let Some(sys) = system {
            total_tokens += (sys.split_whitespace().count() as f32 * 1.3) as usize;
        }

        // Count messages
        for message in messages {
            match &message.content {
                Content::Text { text } => {
                    total_tokens += (text.split_whitespace().count() as f32 * 1.3) as usize;
                }
                _ => {} // Skip non-text content
            }

            // Add overhead for each message
            total_tokens += 4; // Roughly 4 tokens overhead per message
        }

        Ok(TokenUsage {
            input_tokens: total_tokens,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        })
    }
}
