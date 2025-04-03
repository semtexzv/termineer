#![allow(dead_code)]
//! Google Gemini API integration for Termineer
//!
//! Implementation of the LLM provider for Google's Gemini models.
//! Supports Gemini 1.0, 1.5, and 2.0 model families with
//! appropriate token context limits.

use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeSet;
use std::time::Duration;

// Constants for Gemini API
const API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Get the token limit for a Gemini model
///
/// Uses a pattern-matching approach to determine the appropriate token limit
/// for a given model name, based on the model family and version.
fn get_model_token_limit(model_name: &str) -> usize {
    // Default to a conservative limit if no pattern matches
    const DEFAULT_TOKEN_LIMIT: usize = 32_000;

    // Gemini 2.0 models (2M token context)
    if model_name.contains("gemini-2.0") || model_name.starts_with("gemini-2") {
        return 2_097_152; // 2M tokens
    }

    // Gemini 1.5 models (1M token context)
    if model_name.contains("gemini-1.5") || model_name.starts_with("gemini-1.5") {
        return 1_048_576; // 1M tokens
    }

    // Gemini 1.0 Pro models (32K token context)
    if model_name.contains("gemini-1.0-pro") {
        return 32_768; // 32K tokens
    }

    // Fall back to default for any unknown models
    DEFAULT_TOKEN_LIMIT
}

/// Gemini API request types
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<GeminiSafetySetting>>,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxOutputTokens")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topP")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topK")]
    top_k: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", rename = "stopSequences")]
    stop_sequences: Vec<String>,
}

#[derive(Debug, Serialize)]
struct GeminiSafetySetting {
    category: String,
    threshold: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    // Could add support for inlineData/fileData for multimodal content later
}

// Gemini API response types
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    usage_metadata: GeminiUsageMetadata,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: GeminiContent,
    finish_reason: Option<String>,
    safety_ratings: Option<Vec<GeminiSafetyRating>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    prompt_token_count: Option<u32>,
    candidates_token_count: Option<u32>,
    total_token_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GeminiSafetyRating {
    category: String,
    probability: String,
}

// Token counting response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCountTokensResponse {
    total_tokens: Option<u32>,
}

/// Google Gemini API client implementation
pub struct GeminiBackend {
    api_key: String,
    client: reqwest::Client,
    model_name: String,
}

impl GeminiBackend {
    /// Create a new Gemini client
    pub fn new(api_key: String, model_name: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300)) // 5 minute timeout for long context
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_key,
            client,
            model_name,
        }
    }

    /// Convert Termineer message format to Gemini API format
    fn convert_messages_to_gemini_format(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Vec<GeminiContent> {
        let mut gemini_contents = Vec::new();

        // Get the system message either from the parameter or from the messages
        let mut system_text = system.unwrap_or("").to_string();

        // If no system message provided as parameter, look for it in the messages
        if system_text.is_empty() {
            // Find the first system message in the array
            for message in messages {
                if message.role == "system" {
                    if let Content::Text { text } = &message.content {
                        system_text = text.clone();
                        break;
                    }
                }
            }
        }

        let has_system = !system_text.is_empty();

        // Track if we've prepended the system message to a user message
        let mut system_prepended = false;

        // Process messages
        for message in messages {
            let (role, is_user) = match message.role.as_str() {
                "user" => (Some("user"), true),
                "assistant" => (Some("model"), false),
                "system" => continue, // Skip system messages, handled separately
                _ => (None, false),   // Skip tool messages or other unknown roles
            };

            if role.is_none() {
                continue; // Skip this message
            }

            let mut parts = Vec::new();

            // For the first user message, prepend system message if any
            if is_user && has_system && !system_prepended {
                // Extract text from content
                let content_text = extract_text_content(&message.content);
                if let Some(text) = content_text {
                    let mut combined_text = system_text.clone();
                    combined_text.push_str("\n\n");
                    combined_text.push_str(&text);

                    parts.push(GeminiPart {
                        text: Some(combined_text),
                    });

                    system_prepended = true;
                }
            } else {
                // Process content based on type
                let content_text = extract_text_content(&message.content);
                if let Some(text) = content_text {
                    parts.push(GeminiPart { text: Some(text) });
                }
            }

            if !parts.is_empty() {
                gemini_contents.push(GeminiContent {
                    parts,
                    role: role.map(String::from),
                });
            }
        }

        // If we have a system message but no user message to prepend it to
        if has_system && !system_prepended {
            // Create a synthetic user message with the system prompt
            gemini_contents.push(GeminiContent {
                parts: vec![GeminiPart {
                    text: Some(system_text),
                }],
                role: Some("user".to_string()),
            });
        }

        gemini_contents
    }

    /// Send a request to the Gemini API using the standardized retry utility
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        use crate::llm::retry_utils::{send_api_request_with_retry, RetryConfig};

        // Create retry configuration - use linear backoff for Gemini
        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 1000,    // 1 second initial delay
            max_delay_ms: 30000,    // Maximum 30 second delay (per TODO)
            timeout_secs: 180,      // 3 minute timeout (per TODO range of 100-200s)
            use_exponential: false, // Use linear backoff for Gemini
        };

        // Create a request builder closure
        let prepare_request = || {
            self.client
                .post(url)
                .header("Content-Type", "application/json")
                .json(&request_json)
        };

        // Use the standardized retry utility
        send_api_request_with_retry::<T, _>(&self.client, url, prepare_request, config, "Gemini")
            .await
    }
}

// Helper function to extract text content from Content enum
fn extract_text_content(content: &Content) -> Option<String> {
    match content {
        Content::Text { text } => Some(text.clone()),
        _ => None,
    }
}

#[async_trait]
impl Backend for GeminiBackend {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Gemini doesn't support thinking or cache control
        if thinking_budget.is_some() {
            bprintln!(info: "Thinking is not supported by Gemini models, ignoring thinking_budget");
        }

        if cache_points.is_some() {
            bprintln!(info: "Cache points are not supported by Gemini models, ignoring cache_points");
        }

        let gemini_contents = self.convert_messages_to_gemini_format(messages, system);

        // Default max tokens if not provided
        let default_max_tokens = 2048; // Reasonable default
        let tokens = max_tokens.unwrap_or(default_max_tokens);

        let generation_config = GeminiGenerationConfig {
            max_output_tokens: Some(tokens as u32),
            temperature: Some(0.7), // Default temperature
            top_p: Some(0.95),      // Default top_p
            top_k: None,            // Default top_k
            stop_sequences: stop_sequences.map(|seqs| seqs.to_vec()).unwrap_or_default(),
        };

        let request = GeminiRequest {
            contents: gemini_contents,
            generation_config: Some(generation_config),
            safety_settings: None,
        };

        // Construct the API endpoint URL
        let api_url = format!(
            "{}/models/{}:generateContent?key={}",
            API_BASE_URL, self.model_name, self.api_key
        );

        // Use the common send_api_request method
        let gemini_response: GeminiResponse = self
            .send_api_request(&api_url, serde_json::to_value(request).unwrap())
            .await?;

        // Extract the generated text
        if gemini_response.candidates.is_empty() {
            return Err(LlmError::ApiError(
                "No candidates returned from Gemini API".to_string(),
            ));
        }

        let candidate = &gemini_response.candidates[0];
        let mut response_text = String::new();

        for part in &candidate.content.parts {
            if let Some(part_text) = &part.text {
                response_text.push_str(part_text);
            }
        }

        // Extract token usage
        let token_usage = TokenUsage {
            input_tokens: gemini_response
                .usage_metadata
                .prompt_token_count
                .unwrap_or(0) as usize,
            output_tokens: gemini_response
                .usage_metadata
                .candidates_token_count
                .unwrap_or(0) as usize,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        let finish_reason = candidate
            .finish_reason
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        Ok(LlmResponse {
            content: vec![Content::Text {
                text: response_text,
            }],
            usage: Some(token_usage),
            stop_reason: Some(finish_reason),
            stop_sequence: None, // Not provided by Gemini
        })
    }

    async fn count_tokens(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Result<TokenUsage, LlmError> {
        // Convert messages to Gemini format
        let gemini_contents = self.convert_messages_to_gemini_format(messages, system);

        // Prepare request body for countTokens API
        // Note: Gemini countTokens request only needs 'contents'
        let request_body = serde_json::json!({
            "contents": gemini_contents,
        });

        // Construct the API endpoint URL for countTokens
        let api_url = format!(
            "{}/models/{}:countTokens?key={}",
            API_BASE_URL, self.model_name, self.api_key
        );

        // Call the countTokens API using the shared request sender
        let response: GeminiCountTokensResponse = self
            .send_api_request(&api_url, request_body)
            .await
            .map_err(|e| {
                // Add context to the error
                LlmError::ApiError(format!(
                    "Gemini countTokens API call failed for model {}: {}",
                    self.model_name, e
                ))
            })?;

        // Extract the token count from the response
        let total_tokens = response.total_tokens.ok_or_else(|| {
            LlmError::ApiError(format!(
                "Gemini countTokens response for model {} did not contain 'totalTokens'",
                self.model_name
            ))
        })?;

        // Return the result in the TokenUsage struct
        // Note: countTokens only provides the total input tokens. Output is 0 here.
        Ok(TokenUsage {
            input_tokens: total_tokens as usize,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        })
    }

    fn max_token_limit(&self) -> usize {
        get_model_token_limit(&self.model_name)
    }

    fn name(&self) -> &str {
        "gemini"
    }

    fn model(&self) -> &str {
        &self.model_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MessageInfo;

    #[test]
    fn test_message_conversion() {
        let client = GeminiBackend::new("test_key".to_string(), "gemini-2.0-flash".to_string());

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: Content::Text {
                    text: "You are a helpful assistant.".to_string(),
                },
                info: MessageInfo::System,
            },
            Message {
                role: "user".to_string(),
                content: Content::Text {
                    text: "Hello, how are you?".to_string(),
                },
                info: MessageInfo::User,
            },
        ];

        // First test with no external system prompt (uses the one from messages)
        let system_prompt = None;

        let gemini_contents = client.convert_messages_to_gemini_format(&messages, system_prompt);

        assert_eq!(gemini_contents.len(), 1);
        assert_eq!(gemini_contents[0].role, Some("user".to_string()));
        assert_eq!(gemini_contents[0].parts.len(), 1);
        assert_eq!(
            gemini_contents[0].parts[0].text,
            Some("You are a helpful assistant.\n\nHello, how are you?".to_string())
        );

        // Test with an explicit system prompt which should override the one in messages
        let explicit_system = Some("I am an explicit system prompt.");

        let gemini_contents = client.convert_messages_to_gemini_format(&messages, explicit_system);

        assert_eq!(gemini_contents.len(), 1);
        assert_eq!(gemini_contents[0].role, Some("user".to_string()));
        assert_eq!(gemini_contents[0].parts.len(), 1);
        assert_eq!(
            gemini_contents[0].parts[0].text,
            Some("I am an explicit system prompt.\n\nHello, how are you?".to_string())
        );
    }

    #[test]
    fn test_model_token_limits() {
        assert_eq!(get_model_token_limit("gemini-1.0-pro"), 32_768);
        assert_eq!(get_model_token_limit("gemini-1.5-pro-latest"), 1_048_576);
        assert_eq!(get_model_token_limit("gemini-2.0-flash"), 2_097_152);
        assert_eq!(get_model_token_limit("gemini-2.0-pro"), 2_097_152);
    }
}
