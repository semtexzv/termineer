//! DeepSeek API integration for Termineer
//!
//! Implementation of the LLM provider for DeepSeek's models
//! including deepseek-chat (V3) and deepseek-reasoner (R1).

use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeSet;
use std::time::Duration;

// Constants for DeepSeek API
const API_BASE_URL: &str = "https://api.deepseek.com";

/// Get the token limit for a DeepSeek model
///
/// Uses a pattern-matching approach to determine the appropriate token limit
/// for a given model name.
fn get_model_token_limit(model_name: &str) -> usize {
    match model_name {
        "deepseek-chat" => 32_768,     // DeepSeek-V3 (32K context)
        "deepseek-reasoner" => 32_768, // DeepSeek-R1 reasoner model (32K context)
        _ => 16_000,                   // Default to a conservative limit if unknown model
    }
}

/// DeepSeek API request types (Compatible with OpenAI format)
#[derive(Debug, Serialize)]
struct DeepSeekRequest {
    model: String,
    messages: Vec<DeepSeekMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "max_tokens")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "top_p")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stop: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct DeepSeekMessage {
    role: String,
    content: String,
}

// DeepSeek API response types
#[derive(Debug, Deserialize)]
struct DeepSeekResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<DeepSeekChoice>,
    #[serde(default)]
    usage: Option<DeepSeekUsage>,
    #[allow(dead_code)]
    model: String,
    #[serde(default)]
    #[allow(dead_code)]
    object: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChoice {
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
    #[allow(dead_code)]
    index: usize,
    message: DeepSeekChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct DeepSeekChoiceMessage {
    #[allow(dead_code)]
    role: String,
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeepSeekUsage {
    #[serde(rename = "prompt_tokens")]
    prompt_tokens: Option<u32>,
    #[serde(rename = "completion_tokens")]
    completion_tokens: Option<u32>,
    #[serde(rename = "total_tokens")]
    #[allow(dead_code)]
    total_tokens: Option<u32>,
}

/// DeepSeek API client implementation
pub struct DeepSeekBackend {
    api_key: String,
    client: reqwest::Client,
    model_name: String,
}

impl DeepSeekBackend {
    /// Create a new DeepSeek client
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

    /// Convert Termineer messages to DeepSeek format
    fn convert_messages(&self, messages: &[Message], system: Option<&str>) -> Vec<DeepSeekMessage> {
        let mut deepseek_messages = Vec::new();

        // Add system message first if provided
        if let Some(system_content) = system {
            deepseek_messages.push(DeepSeekMessage {
                role: "system".to_string(),
                content: system_content.to_string(),
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
                "tool" => "tool",     // DeepSeek supports tool messages with function calling
                _ => continue,        // Skip unknown roles
            };

            // Convert content based on type
            let content = match &message.content {
                Content::Text { text } => text.clone(),
                Content::Thinking { thinking, .. } => thinking.clone().unwrap_or_default(),
                Content::RedactedThinking { data } => data.clone().unwrap_or_default(),
                Content::Document { source } => source.clone(),
                Content::Image { .. } => {
                    // DeepSeek doesn't support image inputs in messages
                    // Skip this message or include a placeholder
                    "[Image content not supported]".to_string()
                }
            };

            deepseek_messages.push(DeepSeekMessage {
                role: role.to_string(),
                content,
            });
        }

        deepseek_messages
    }

    /// Send a request to the DeepSeek API using the standardized retry utility
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        use crate::llm::retry_utils::{send_api_request_with_retry, RetryConfig};

        // Create retry configuration - use linear backoff for DeepSeek
        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 1000,    // 1 second initial delay
            max_delay_ms: 30000,    // Maximum 30 second delay (per TODO)
            timeout_secs: 180,      // 3 minute timeout (per TODO range of 100-200s)
            use_exponential: false, // Use linear backoff for DeepSeek
        };

        // Construct the API URL
        let api_url = format!("{API_BASE_URL}{endpoint}");

        // Create a request builder closure that includes all necessary headers
        let prepare_request = || {
            self.client
                .post(&api_url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", &self.api_key))
                .json(&request_json)
        };

        // Use the standardized retry utility
        send_api_request_with_retry::<T, _>(
            &self.client,
            &api_url,
            prepare_request,
            config,
            "DeepSeek",
        )
        .await
    }
}

#[async_trait]
impl Backend for DeepSeekBackend {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>,
        cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // DeepSeek doesn't support thinking or cache features
        if thinking_budget.is_some() {
            // bprintln!(info: "Thinking is not supported by DeepSeek, ignoring thinking_budget");
        }

        if cache_points.is_some() {
            // bprintln!(info: "Cache points are not supported by DeepSeek, ignoring cache_points");
        }

        // Convert messages to DeepSeek format
        let deepseek_messages = self.convert_messages(messages, system);

        // Set up stop sequences if provided
        let stop = stop_sequences.map(|seqs| seqs.to_vec()).unwrap_or_default();

        // Create the request
        let request = DeepSeekRequest {
            model: self.model_name.clone(),
            messages: deepseek_messages,
            temperature: Some(0.7), // Default temperature
            max_tokens: max_tokens.map(|t| t as u32),
            top_p: Some(0.95), // Default top_p
            stop,
            stream: None, // Not using streaming in this implementation
        };

        // Send the request to the chat completions endpoint
        let deepseek_response: DeepSeekResponse = self
            .send_api_request("/chat/completions", serde_json::to_value(request).unwrap())
            .await?;

        // Extract the generated text
        if deepseek_response.choices.is_empty() {
            return Err(LlmError::ApiError(
                "No choices returned from DeepSeek API".to_string(),
            ));
        }

        let choice = &deepseek_response.choices[0];
        let response_text = choice.message.content.clone().unwrap_or_default();

        // For the reasoning model, we might have reasoning content
        let reasoning_text = choice.message.reasoning_content.clone();

        // Prepare final content
        let content = if self.model_name == "deepseek-reasoner" && reasoning_text.is_some() {
            // For deepseek-reasoner, we include both reasoning and final answer
            let reasoning = reasoning_text.unwrap_or_default();
            vec![
                Content::Thinking {
                    signature: None,
                    thinking: Some(reasoning.clone()),
                },
                Content::Text {
                    text: response_text,
                },
            ]
        } else {
            // Regular response for other models
            vec![Content::Text {
                text: response_text,
            }]
        };

        // Extract token usage
        let token_usage = if let Some(usage) = &deepseek_response.usage {
            TokenUsage {
                input_tokens: usage.prompt_tokens.unwrap_or(0) as usize,
                output_tokens: usage.completion_tokens.unwrap_or(0) as usize,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        } else {
            // Approximate token usage if not provided
            let output_len = content
                .iter()
                .map(|c| match c {
                    Content::Text { text } => text.len(),
                    Content::Thinking { thinking, .. } => thinking.as_ref().map_or(0, |t| t.len()),
                    _ => 0,
                })
                .sum::<usize>()
                / 4; // Rough estimate

            TokenUsage {
                input_tokens: 0,
                output_tokens: output_len,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        };

        // Extract finish reason
        let finish_reason = choice
            .finish_reason
            .clone()
            .unwrap_or_else(|| "unknown".to_string());

        Ok(LlmResponse {
            content,
            usage: Some(token_usage),
            stop_reason: Some(finish_reason),
            stop_sequence: None, // Not provided by API
        })
    }


    fn max_token_limit(&self) -> usize {
        get_model_token_limit(&self.model_name)
    }

    fn name(&self) -> &str {
        "deepseek"
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
        let client = DeepSeekBackend::new("test_key".to_string(), "deepseek-chat".to_string());

        let messages = vec![Message {
            role: "user".to_string(),
            content: Content::Text {
                text: "Hello, how are you?".to_string(),
            },
            info: MessageInfo::User,
        }];

        let system_prompt = Some("You are a helpful assistant.");

        let deepseek_messages = client.convert_messages(&messages, system_prompt);

        assert_eq!(deepseek_messages.len(), 2);
        assert_eq!(deepseek_messages[0].role, "system");
        assert_eq!(deepseek_messages[0].content, "You are a helpful assistant.");
        assert_eq!(deepseek_messages[1].role, "user");
        assert_eq!(deepseek_messages[1].content, "Hello, how are you?");
    }

    #[test]
    fn test_model_token_limits() {
        assert_eq!(get_model_token_limit("deepseek-chat"), 32_768);
        assert_eq!(get_model_token_limit("deepseek-reasoner"), 32_768);
        assert_eq!(get_model_token_limit("unknown-model"), 16_000);
    }
}
