//! OpenRouter API integration for Termineer
//! 
//! Implementation of the LLM provider for OpenRouter's unified API
//! which provides access to models from multiple providers including
//! OpenAI, Anthropic, and more.

use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeSet;
use std::time::Duration;
use tokio::time::sleep;

// Constants for OpenRouter API
const API_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// Get the token limit for an OpenRouter model
///
/// Uses a pattern-matching approach to determine the appropriate token limit
/// for a given model name, based on the provider and model.
fn get_model_token_limit(model_name: &str) -> usize {
    // Default to a conservative limit if no pattern matches
    const DEFAULT_TOKEN_LIMIT: usize = 16_000;

    // Extract provider and model parts if in provider/model format
    let parts: Vec<&str> = model_name.split('/').collect();
    let (provider, model) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        // If no provider prefix, assume it's just a model name
        ("", model_name)
    };

    // Handle OpenAI models
    if provider == "openai" || model.starts_with("gpt-") {
        if model.contains("gpt-4-turbo") || model.contains("gpt-4o") {
            return 128_000; // 128k tokens
        } else if model.contains("gpt-4-32k") {
            return 32_768; // 32k tokens
        } else if model.contains("gpt-4") {
            return 8_192; // 8k tokens
        } else if model.contains("gpt-3.5-turbo-16k") {
            return 16_384; // 16k tokens
        } else if model.contains("gpt-3.5") {
            return 4_096; // 4k tokens
        }
    }

    // Handle Anthropic models
    if provider == "anthropic" || model.starts_with("claude-") {
        if model.contains("claude-3-opus") {
            return 200_000; // 200k tokens
        } else if model.contains("claude-3-sonnet") {
            return 200_000; // 200k tokens
        } else if model.contains("claude-3-haiku") {
            return 200_000; // 200k tokens
        } else if model.contains("claude-2") {
            return 100_000; // 100k tokens
        } else if model.contains("claude-instant") {
            return 100_000; // 100k tokens
        }
    }

    // For any other model, use a reasonable default
    DEFAULT_TOKEN_LIMIT
}

/// OpenRouter API request types
#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "max_tokens")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "top_p")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OpenRouterMessage {
    role: String,
    content: OpenRouterContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenRouterContent {
    Text(String),
    Parts(Vec<OpenRouterContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum OpenRouterContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenRouterImageUrl },
}

#[derive(Debug, Serialize)]
struct OpenRouterImageUrl {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

// OpenRouter API response types
#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    id: String,
    choices: Vec<OpenRouterChoice>,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
    model: String,
    #[serde(default)]
    object: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    #[serde(default)]
    index: Option<usize>,
    message: OpenRouterChoiceMessage,
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoiceMessage {
    role: String,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    #[serde(rename = "prompt_tokens")]
    prompt_tokens: Option<u32>,
    #[serde(rename = "completion_tokens")]
    completion_tokens: Option<u32>,
    #[serde(rename = "total_tokens")]
    total_tokens: Option<u32>,
}

/// OpenRouter API client implementation
pub struct OpenRouterBackend {
    api_key: String,
    client: reqwest::Client,
    model_name: String,
    site_url: Option<String>,
    site_name: Option<String>,
}

impl OpenRouterBackend {
    /// Create a new OpenRouter client
    pub fn new(
        api_key: String, 
        model_name: String,
        site_url: Option<String>,
        site_name: Option<String>
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout for long context
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_key,
            client,
            model_name,
            site_url,
            site_name,
        }
    }

    /// Convert Termineer messages to OpenRouter format
    fn convert_messages(&self, messages: &[Message], system: Option<&str>) -> Vec<OpenRouterMessage> {
        let mut openrouter_messages = Vec::new();

        // Add system message first if provided
        if let Some(system_content) = system {
            openrouter_messages.push(OpenRouterMessage {
                role: "system".to_string(),
                content: OpenRouterContent::Text(system_content.to_string()),
                name: None,
            });
        }

        // Process the rest of the messages
        for message in messages {
            // Skip system messages already handled separately
            if message.role == "system" {
                continue;
            }

            // Map message role
            let role = match message.role.as_str() {
                "user" => "user",
                "assistant" => "assistant",
                "system" => continue, // Skip, already handled
                "tool" => "tool", // OpenRouter supports tool messages
                _ => continue,     // Skip unknown roles
            };

            // Convert content based on type
            let content = match &message.content {
                Content::Text { text } => OpenRouterContent::Text(text.clone()),
                Content::Image { source } => {
                    // Support for image sources
                    match source {
                        crate::llm::ImageSource::Base64 { data, media_type } => {
                            let base64_url = format!("data:{};base64,{}", media_type, data);
                            let parts = vec![
                                OpenRouterContentPart::ImageUrl { 
                                    image_url: OpenRouterImageUrl { 
                                        url: base64_url, 
                                        detail: None 
                                    }
                                }
                            ];
                            OpenRouterContent::Parts(parts)
                        }
                    }
                },
                // Other content types not currently supported in multimodal format
                Content::Thinking { thinking, .. } => {
                    OpenRouterContent::Text(thinking.clone().unwrap_or_default())
                },
                Content::RedactedThinking { data } => {
                    OpenRouterContent::Text(data.clone().unwrap_or_default())
                },
                Content::Document { source } => {
                    OpenRouterContent::Text(source.clone())
                }
            };

            // No user info in MessageInfo in this codebase - just pass None for name
            let name = None;

            openrouter_messages.push(OpenRouterMessage {
                role: role.to_string(),
                content,
                name,
            });
        }

        openrouter_messages
    }

    /// Send a request to the OpenRouter API with improved timeout and retry logic
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        // Retry configuration
        let mut attempts = 0;
        let max_attempts = 5;
        let base_retry_delay_ms = 1000; // 1 second initial retry delay
        let max_retry_delay_ms = 30000; // Maximum 30 second retry delay
        let request_timeout = Duration::from_secs(180); // 3 minute timeout

        // Construct the API URL
        let api_url = format!("{}{}", API_BASE_URL, endpoint);

        loop {
            // Log the retry attempt if not the first attempt
            if attempts > 0 {
                bprintln!(warn: "🔄 Retry attempt {} of {} for OpenRouter API call", attempts, max_attempts);
            }

            // Build the request with timeout and headers
            let mut request = self
                .client
                .post(&api_url)
                .timeout(request_timeout)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key));

            // Add optional discovery headers if provided
            if let Some(url) = &self.site_url {
                request = request.header("HTTP-Referer", url);
            }
            if let Some(name) = &self.site_name {
                request = request.header("X-Title", name);
            }

            // Add the request body
            request = request.json(&request_json);
            
            // Send the request
            let response = request.send().await;

            match response {
                Ok(res) => {
                    if res.status().is_success() {
                        return res.json::<T>().await.map_err(|e| {
                            LlmError::ApiError(format!("Failed to parse OpenRouter response: {}", e))
                        });
                    } else if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                        // Handle rate limiting (429 Too Many Requests)
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(LlmError::RateLimitError { 
                                retry_after: None 
                            });
                        }

                        // Linear backoff
                        let linear_delay = base_retry_delay_ms * (attempts as u64);
                        let capped_delay = linear_delay.min(max_retry_delay_ms);
                        bprintln!(warn: "⏱️ Rate limit exceeded. Retrying in {} seconds", capped_delay / 1000);
                        
                        sleep(Duration::from_millis(capped_delay)).await;
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
                                "Max retries reached. OpenRouter server error {}: {}",
                                status, error_text
                            )));
                        }
                        
                        // Linear backoff for server errors
                        let linear_delay = base_retry_delay_ms * (attempts as u64);
                        let capped_delay = linear_delay.min(max_retry_delay_ms);
                        
                        bprintln!(error: "⚠️ OpenRouter API server error {}. Retrying in {} seconds (attempt {}/{})",
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
                            "OpenRouter HTTP error {}: {}",
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
                        bprintln!("⏱️ OpenRouter API request timed out. Retrying in {} seconds (attempt {}/{})",
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

#[async_trait]
impl Backend for OpenRouterBackend {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // OpenRouter doesn't support thinking or cache features
        if thinking_budget.is_some() {
            bprintln!(info: "Thinking is not supported by OpenRouter, ignoring thinking_budget");
        }
        
        if cache_points.is_some() {
            bprintln!(info: "Cache points are not supported by OpenRouter, ignoring cache_points");
        }
        
        // Convert messages to OpenRouter format
        let openrouter_messages = self.convert_messages(messages, system);
        
        // Set up stop sequences if provided
        let stop = stop_sequences
            .map(|seqs| seqs.to_vec())
            .unwrap_or_default();
        
        // Create the request
        let request = OpenRouterRequest {
            model: Some(self.model_name.clone()),
            messages: openrouter_messages,
            stream: None, // Not using streaming in this implementation
            max_tokens: max_tokens.map(|t| t as u32),
            temperature: Some(0.7), // Default temperature
            top_p: Some(0.95), // Default top_p
            stop,
            seed: None, // No deterministic seed by default
        };

        // Send the request to the chat completions endpoint
        let openrouter_response: OpenRouterResponse = 
            self.send_api_request("/chat/completions", serde_json::to_value(request).unwrap()).await?;

        // Extract the generated text
        if openrouter_response.choices.is_empty() {
            return Err(LlmError::ApiError("No choices returned from OpenRouter API".to_string()));
        }

        let choice = &openrouter_response.choices[0];
        let response_text = choice.message.content.clone().unwrap_or_default();

        // Extract token usage
        let token_usage = if let Some(usage) = &openrouter_response.usage {
            TokenUsage {
                input_tokens: usage.prompt_tokens.unwrap_or(0) as usize,
                output_tokens: usage.completion_tokens.unwrap_or(0) as usize,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        } else {
            // Approximate token usage if not provided
            TokenUsage {
                input_tokens: 0,
                output_tokens: response_text.len() / 4, // Rough estimate
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        };

        // Extract finish reason
        let finish_reason = choice.finish_reason.clone().unwrap_or_else(|| "unknown".to_string());

        Ok(LlmResponse {
            content: vec![Content::Text { text: response_text }],
            usage: Some(token_usage),
            stop_reason: Some(finish_reason),
            stop_sequence: None, // Not provided by API
        })
    }

    async fn count_tokens(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Result<TokenUsage, LlmError> {
        // Use a simple character-based estimation
        // This is a rough approximation since OpenRouter doesn't provide a token counting endpoint
        let estimate_tokens: usize = messages.iter()
            .map(|msg| {
                match &msg.content {
                    Content::Text { text } => text.len() / 4, // Rough estimate: ~4 chars per token
                    Content::Image { .. } => 1000, // Rough estimate for images 
                    Content::Thinking { thinking, .. } => thinking.as_ref().map_or(0, |t| t.len() / 4),
                    Content::RedactedThinking { data } => data.as_ref().map_or(0, |d| d.len() / 4),
                    Content::Document { source } => source.len() / 4,
                }
            })
            .sum();
            
        let sys_tokens: usize = system.map_or(0, |sys| sys.len() / 4);
        
        Ok(TokenUsage {
            input_tokens: estimate_tokens + sys_tokens,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        })
    }

    fn max_token_limit(&self) -> usize {
        get_model_token_limit(&self.model_name)
    }

    fn name(&self) -> &str {
        "openrouter"
    }

    fn model(&self) -> &str {
        &self.model_name
    }
}
