//! OpenAI API Provider
//!
//! Implementation of the LLM provider for OpenAI's models (GPT-3.5, GPT-4, etc.).

use crate::llm::{Backend, Content, LlmError, LlmResponse, Message, TokenUsage};
#[cfg(test)]
use crate::llm::ImageSource;
use crate::llm::retry_utils;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::BTreeSet;
use std::time::Duration;

// Constants for OpenAI API
const API_BASE_URL: &str = "https://api.openai.com/v1";

/// Get the token limit for an OpenAI model
///
/// Uses a pattern-matching approach to determine the appropriate token limit
/// for a given model name. Limits based on OpenAI documentation.
fn get_model_token_limit(model_name: &str) -> usize {
    // Default to a conservative limit if no pattern matches
    const DEFAULT_TOKEN_LIMIT: usize = 8_000;

    // GPT-4o models (128k context)
    if model_name.starts_with("gpt-4o") {
        return 128_000;
    }
    // GPT-4 Turbo models (128k context)
    if model_name.starts_with("gpt-4-turbo") || model_name.starts_with("gpt-4-1106") || model_name.starts_with("gpt-4-0125") {
        return 128_000;
    }
    // GPT-4 32k models
    if model_name.starts_with("gpt-4-32k") {
        return 32_768;
    }
    // Standard GPT-4 models (8k context)
    if model_name.starts_with("gpt-4") {
        return 8_192;
    }
    // GPT-3.5 Turbo 16k models (older naming)
    if model_name.starts_with("gpt-3.5-turbo-16k") {
        return 16_384;
    }
    // Standard GPT-3.5 Turbo models (16k context in newer versions)
    if model_name.starts_with("gpt-3.5-turbo") {
        // Newer gpt-3.5-turbo models default to 16k, older was 4k. Assume 16k.
        return 16_384;
    }

    // Fall back to default for any unknown models
    DEFAULT_TOKEN_LIMIT
}

/// OpenAI API request structure (Completions)
#[derive(Debug, Serialize)]
struct OpenAICompletionRequest {
    model: String,
    prompt: String,
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
    // Add other parameters like frequency_penalty, presence_penalty if needed
}




/// OpenAI API response structure (Completions)
#[derive(Debug, Deserialize)]
struct OpenAICompletionResponse {
    #[allow(dead_code)]
    id: String,
    choices: Vec<OpenAICompletionChoice>,
    #[serde(default)]
    usage: Option<OpenAIUsage>,
    #[allow(dead_code)]
    model: String,
    #[serde(default)]
    #[allow(dead_code)]
    object: Option<String>,
    #[allow(dead_code)]
    system_fingerprint: Option<String>, // Added system_fingerprint
}

#[derive(Debug, Deserialize)]
struct OpenAICompletionChoice {
    #[serde(default)]
    #[allow(dead_code)]
    index: Option<usize>,
    text: String, // Direct text response in completions API
    #[serde(rename = "finish_reason")]
    finish_reason: Option<String>,
    #[allow(dead_code)]
    logprobs: Option<serde_json::Value>, // Added logprobs
}




#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    #[serde(rename = "prompt_tokens")]
    prompt_tokens: Option<u32>,
    #[serde(rename = "completion_tokens")]
    completion_tokens: Option<u32>,
    #[serde(rename = "total_tokens")]
    #[allow(dead_code)]
    total_tokens: Option<u32>,
}

/// Implementation of LLM provider for OpenAI
pub struct OpenAIBackend {
    api_key: String,
    client: reqwest::Client,
    model_name: String,
}

impl OpenAIBackend {
    /// Create a new OpenAI client
    pub fn new(api_key: String, model_name: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180)) // Standard timeout
            .build()
            .expect("Failed to create HTTP client");

        Self {
            api_key,
            client,
            model_name,
        }
    }

    /// Convert Termineer messages to a single prompt string for completions API
    fn convert_messages_to_prompt(&self, messages: &[Message], system: Option<&str>) -> String {
        let mut prompt_parts = Vec::new();

        // Add system message first if provided
        if let Some(system_content) = system {
            prompt_parts.push(format!("System: {}", system_content));
        }

        // Process the rest of the messages
        for message in messages {
            // Skip system messages already handled separately
            if message.role == "system" {
                continue;
            }

            // Map message role to a prompt format
            let role_prefix = match message.role.as_str() {
                "user" => "Human",
                "assistant" => "Assistant", 
                "tool" => "Tool",
                _ => continue, // Skip unknown roles
            };

            // Convert content based on type
            let content_text = match &message.content {
                Content::Text { text } => text.clone(),
                Content::Image { source: _ } => {
                    // Note: Completions API doesn't support images directly
                    // Convert to text description
                    "[Image content - not supported in completions API]".to_string()
                }
                Content::Thinking { thinking, .. } => format!("[Thinking]: {}", thinking.clone().unwrap_or_default()),
                Content::RedactedThinking { data } => format!("[Redacted Thinking]: {}", data.clone().unwrap_or_default()),
                Content::Document { source } => format!("[Document Source]: {}", source.clone()),
            };

            prompt_parts.push(format!("{}: {}", role_prefix, content_text));
        }

        // Join all parts with newlines and add a final prompt for the assistant
        let mut prompt = prompt_parts.join("\n\n");
        if !prompt.is_empty() {
            prompt.push_str("\n\nAssistant:");
        }

        prompt
    }

    /// Send a request to the OpenAI API using the standardized retry utility
    async fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        // Use standard retry configuration (linear backoff, 180s timeout)
        let config = retry_utils::create_standard_retry_config();

        // Construct the API URL
        let api_url = format!("{}{}", API_BASE_URL, endpoint);

        // Create a request builder closure that includes all necessary headers
        let prepare_request = || {
            self.client
                .post(&api_url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request_json)
        };

        // Use the standardized retry utility
        retry_utils::send_api_request_with_retry::<T, _>(
            &self.client,
            &api_url,
            prepare_request,
            config,
            "OpenAI",
        )
        .await
    }
}

#[async_trait]
impl Backend for OpenAIBackend {
    async fn send_message(
        &self,
        messages: &[Message],
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        thinking_budget: Option<usize>, // OpenAI doesn't use this
        cache_points: Option<&BTreeSet<usize>>, // OpenAI doesn't use this
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Log unsupported features if used
        if thinking_budget.is_some() {
            bprintln!(dev: "Thinking budget is not supported by OpenAI, ignoring.");
        }
        if cache_points.is_some() {
            bprintln!(dev: "Cache points are not supported by OpenAI, ignoring.");
        }

        // Convert messages to a single prompt string
        let prompt = self.convert_messages_to_prompt(messages, system);

        // Set up stop sequences if provided
        let stop = stop_sequences.map(|seqs| seqs.to_vec()).unwrap_or_default();

        // Create the request
        let request = OpenAICompletionRequest {
            model: self.model_name.clone(),
            prompt,
            temperature: Some(0.7), // Default temperature
            max_tokens: max_tokens.map(|t| t as u32),
            top_p: Some(1.0), // Default top_p for OpenAI
            stop,
            stream: None, // Not using streaming
        };

        // Send the request to the completions endpoint
        let openai_response: OpenAICompletionResponse = self
            .send_api_request("/completions", serde_json::to_value(request).unwrap())
            .await?;

        // Extract the generated text
        if openai_response.choices.is_empty() {
            return Err(LlmError::ApiError(
                "No choices returned from OpenAI API".to_string(),
            ));
        }

        // Process the first choice
        let choice = &openai_response.choices[0];
        let response_text = choice.text.clone();

        // Prepare final content (currently only text)
        let content = vec![Content::Text { text: response_text }];

        // Extract token usage
        let token_usage = if let Some(usage) = &openai_response.usage {
            TokenUsage {
                input_tokens: usage.prompt_tokens.unwrap_or(0) as usize,
                output_tokens: usage.completion_tokens.unwrap_or(0) as usize,
                cache_creation_input_tokens: 0, // Not supported
                cache_read_input_tokens: 0,     // Not supported
            }
        } else {
            // Approximate token usage if not provided (less likely with OpenAI)
            TokenUsage {
                input_tokens: 0,
                output_tokens: content.iter().map(|c| match c {
                    Content::Text { text } => text.len() / 4, // Rough estimate
                    _ => 0,
                }).sum(),
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }
        };

        // Extract finish reason and potential stop sequence
        let finish_reason = choice.finish_reason.clone();
        let stop_sequence = if finish_reason.as_deref() == Some("stop") {
            // OpenAI doesn't directly return the sequence that caused the stop in the main response.
            // We might infer it if needed, but for now, return None.
             None
        } else {
            None
        };

        Ok(LlmResponse {
            content,
            usage: Some(token_usage),
            stop_reason: finish_reason,
            stop_sequence,
        })
    }

    fn max_token_limit(&self) -> usize {
        get_model_token_limit(&self.model_name)
    }

    fn name(&self) -> &str {
        "openai"
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
    fn test_openai_prompt_conversion() {
        let client = OpenAIBackend::new("test_key".to_string(), "gpt-4".to_string());

        let messages = vec![
            Message {
                role: "user".to_string(),
                content: Content::Text { text: "Hello".to_string() },
                info: MessageInfo::User,
            },
            Message {
                role: "assistant".to_string(),
                content: Content::Text { text: "Hi there!".to_string() },
                info: MessageInfo::Assistant,
            },
            Message {
                role: "user".to_string(),
                content: Content::Text { text: "How are you?".to_string() },
                info: MessageInfo::User,
            }
        ];

        let system_prompt = Some("You are a helpful assistant.");

        let prompt = client.convert_messages_to_prompt(&messages, system_prompt);

        let expected = "System: You are a helpful assistant.\n\nHuman: Hello\n\nAssistant: Hi there!\n\nHuman: How are you?\n\nAssistant:";
        assert_eq!(prompt, expected);
    }

    #[test]
    fn test_openai_prompt_conversion_no_system() {
        let client = OpenAIBackend::new("test_key".to_string(), "gpt-4".to_string());

        let messages = vec![
            Message {
                role: "user".to_string(),
                content: Content::Text { text: "Hello".to_string() },
                info: MessageInfo::User,
            }
        ];

        let prompt = client.convert_messages_to_prompt(&messages, None);

        let expected = "Human: Hello\n\nAssistant:";
        assert_eq!(prompt, expected);
    }

    #[test]
    fn test_openai_prompt_conversion_with_image() {
        let client = OpenAIBackend::new("test_key".to_string(), "gpt-4".to_string());

        let messages = vec![
            Message {
                role: "user".to_string(),
                content: Content::Image { source: ImageSource::Base64 { media_type: "image/png".to_string(), data: "base64data".to_string() }},
                info: MessageInfo::User,
            }
        ];

        let prompt = client.convert_messages_to_prompt(&messages, None);

        let expected = "Human: [Image content - not supported in completions API]\n\nAssistant:";
        assert_eq!(prompt, expected);
    }

    #[test]
    fn test_openai_model_token_limits() {
        assert_eq!(get_model_token_limit("gpt-4o-2024-05-13"), 128_000);
        assert_eq!(get_model_token_limit("gpt-4-turbo"), 128_000);
        assert_eq!(get_model_token_limit("gpt-4-turbo-preview"), 128_000);
        assert_eq!(get_model_token_limit("gpt-4-0125-preview"), 128_000);
        assert_eq!(get_model_token_limit("gpt-4-1106-preview"), 128_000);
        assert_eq!(get_model_token_limit("gpt-4-32k"), 32_768);
        assert_eq!(get_model_token_limit("gpt-4-0613"), 8_192);
        assert_eq!(get_model_token_limit("gpt-4"), 8_192);
        assert_eq!(get_model_token_limit("gpt-3.5-turbo-16k"), 16_384);
        assert_eq!(get_model_token_limit("gpt-3.5-turbo-0125"), 16_384);
        assert_eq!(get_model_token_limit("gpt-3.5-turbo"), 16_384); // Assuming newer 16k default
        assert_eq!(get_model_token_limit("unknown-model"), 8_000); // Default
    }
}