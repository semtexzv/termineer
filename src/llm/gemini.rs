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
use std::sync::Mutex;
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
#[serde(rename_all = "camelCase")] // Add rename_all here too for consistency
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<GeminiSafetySetting>>,
    /// The name of the cached content to use (e.g., "cachedContents/xxxxxxxx")
    #[serde(skip_serializing_if = "Option::is_none", rename = "cachedContent")]
    cached_content: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq)] // Added Clone and PartialEq
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Added Clone and PartialEq
struct GeminiContent {
    parts: Vec<GeminiPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Added Clone and PartialEq
struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    // Could add support for inlineData/fileData for multimodal content later
}

// Gemini API response types

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PromptFeedback {
    block_reason: Option<String>,
    // We can add safety_ratings here later if needed
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    // Use serde(default) for candidates in case it's missing when blocked
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    usage_metadata: GeminiUsageMetadata,
    #[serde(default)] // Add default for promptFeedback as well
    prompt_feedback: Option<PromptFeedback>,
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
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: Option<u32>,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: Option<u32>,
    #[serde(default, rename = "cachedContentTokenCount")] // Add default in case it's missing in some responses
    cached_content_token_count: Option<u32>,
    #[serde(rename = "totalTokenCount")]
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
    // Store the name and number of messages included in the most recent cache
    last_cache_info: Mutex<Option<(String, usize)>>, // (cache_name, num_messages_cached)
}

// Structs for Gemini Context Caching API
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCreateCacheRequest {
    cached_content: GeminiCachedContentInput,
    // ttl: Option<String>, // Optional: e.g., "3600s" for 1 hour
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCachedContentInput {
    model: String, // Model the cache is associated with, e.g., "models/gemini-1.5-pro-latest"
    contents: Vec<GeminiContent>,
    system_instruction: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCachedContentResponse {
    name: String, // Format: "cachedContents/xxxxxxxx"
    // Includes other fields like createTime, updateTime, expireTime, model, usageMetadata etc.
    // but we only need the name for now.
    usage_metadata: GeminiUsageMetadata,
}

// Struct for counting tokens
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCountTokensRequest {
    contents: Vec<GeminiContent>,
    // system_instruction is NOT part of the countTokens request body according to docs/API behavior
    model: String, // Model name is required for counting
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
            last_cache_info: Mutex::new(None), // Initialize cache info as empty
        }
    }

    /// Convert Termineer message format to Gemini API format
    /// Does NOT include the system prompt, which is handled separately.
    fn convert_messages_to_gemini_format(&self, messages: &[Message]) -> Vec<GeminiContent> {
        let mut gemini_contents = Vec::new();

        // Process messages, skipping system prompts
        for message in messages {
            let role = match message.role.as_str() {
                "user" => Some("user"),
                "assistant" => Some("model"),
                "system" => continue, // Skip system messages
                _ => None,            // Skip tool messages or other unknown roles
            };

            if role.is_none() {
                continue; // Skip this message
            }

            // Process content based on type
            let content_text = extract_text_content(&message.content);
            if let Some(text) = content_text {
                let parts = vec![GeminiPart { text: Some(text) }];
                gemini_contents.push(GeminiContent {
                    parts,
                    role: role.map(String::from),
                });
            }
            // Note: Could add image handling here if needed in the future
        }

        // Ensure the conversation starts with a 'user' role if the first message isn't 'user'
        // and the list is not empty. Gemini requires alternating roles starting with 'user'.
        if let Some(first_content) = gemini_contents.first() {
            if first_content.role != Some("user".to_string()) {
                // Prepend an empty user message if the first message is from the model
                gemini_contents.insert(
                    0,
                    GeminiContent {
                        parts: vec![GeminiPart {
                            text: Some("".to_string()),
                        }], // Add an empty text part
                        role: Some("user".to_string()),
                    },
                );
            }
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
            max_attempts: 30,
            base_delay_ms: 1000,    // 1 second initial delay
            max_delay_ms: 30000,    // Maximum 30 second delay (per TODO)
            timeout_secs: 180,      // 3 minute timeout (per TODO range of 100-200s)
            use_exponential: true, // Use linear backoff for Gemini
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

    /// Count tokens for the given messages and system prompt.
    async fn count_tokens(
        &self,
        contents: &[GeminiContent],
        system_instruction: Option<&GeminiContent>,
    ) -> Result<usize, LlmError> {
        let request = GeminiRequest {
            contents: contents.to_vec(),
            system_instruction: None,
            generation_config: None,
            safety_settings: None,
            cached_content: None,
        };

        let api_url = format!(
            "{}/models/{}:countTokens?key={}",
            API_BASE_URL, self.model_name, self.api_key
        );

        let response: GeminiCountTokensResponse = self
            .send_api_request(&api_url, serde_json::to_value(request).unwrap())
            .await?;

        Ok(response.total_tokens.unwrap_or(0) as usize)
    }

    /// Create a CachedContent resource on the Gemini API.
    async fn create_cached_content(
        &self,
        contents_to_cache: Vec<GeminiContent>,
        system_instruction_to_cache: Option<GeminiContent>,
    ) -> Result<(String, usize, TokenUsage), LlmError> { // Returns (cache_name, num_messages_cached, creation_token_usage)
        let model_for_cache = format!("models/{}", self.model_name); // Use the full model path
        let num_messages_cached = contents_to_cache.len(); // Count messages being cached

        let cache_input = GeminiCachedContentInput {
            model: model_for_cache.clone(),
            contents: contents_to_cache,
            system_instruction: system_instruction_to_cache,
        };

        let api_url = format!("{}/cachedContents?key={}", API_BASE_URL, self.api_key);

        let response: GeminiCachedContentResponse = self
            .send_api_request(&api_url, serde_json::to_value(cache_input).unwrap())
            .await?;

        // Extract token usage from the cache creation response
        // Assume totalTokenCount reflects the tokens written to the cache
        let creation_token_usage = TokenUsage {
             input_tokens: 0, // Not applicable for cache creation itself
             output_tokens: 0, // Not applicable for cache creation itself
             cache_read_input_tokens: 0, // No cache read during creation
             cache_creation_input_tokens: response.usage_metadata.total_token_count.unwrap_or(0) as usize,
        };

        bprintln!(dev: "Gemini Cache: Created CachedContent '{}' with {} messages. Tokens written to cache: {}",
                 response.name, num_messages_cached, creation_token_usage.cache_creation_input_tokens);

        // Store the new cache info (name and count)
        {
            let mut cache_guard = self.last_cache_info.lock().unwrap();
            *cache_guard = Some((response.name.clone(), num_messages_cached));
        }

        Ok((response.name, num_messages_cached, creation_token_usage))
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
            // bprintln!(info: "Thinking is not supported by Gemini models, ignoring thinking_budget");
        }

        if cache_points.is_some() {
            // bprintln!(dev: "Gemini Cache: Note - BTreeSet cache_points are ignored by Gemini provider.");
        }

        // --- Gemini Context Caching Logic ---
        const CACHE_TOKEN_THRESHOLD: usize = 25000;

        // Convert *all* messages and system prompt
        let all_gemini_contents = self.convert_messages_to_gemini_format(messages);
        let system_instruction_content = system.map(|s| GeminiContent {
            parts: vec![GeminiPart { text: Some(s.to_string()) }],
            role: None,
        });

        let mut request_contents = all_gemini_contents.clone(); // Start assuming we send everything
        let mut request_system_instruction = system_instruction_content.clone();
        let mut cached_content_name: Option<String> = None;
        let mut use_cache = false;
        let mut cache_creation_tokens = 0; // Track tokens used for cache creation

        // Check if the model name indicates a preview or experimental version
        let model_name_lower = self.model_name.to_lowercase();
        let is_preview_or_exp = model_name_lower.contains("preview") || model_name_lower.contains("exp");

        if is_preview_or_exp {
             bprintln!(dev: "Gemini Cache: Skipping caching for preview/experimental model '{}'", self.model_name);
             // Ensure cache is not used and no creation tokens are counted
             use_cache = false; // Explicitly set to false
             cached_content_name = None; // Ensure no cache name is used
             cache_creation_tokens = 0; // Ensure no creation tokens are counted
             // request_contents and request_system_instruction remain the full conversation
        } else {
            // --- Original Caching Logic Start ---
            // Check if we can reuse an existing cache
            if let Some((ref existing_cache_name, num_cached)) = *self.last_cache_info.lock().unwrap() {
                if num_cached < all_gemini_contents.len() {
                    // Check if the cached part matches the current conversation start
                    if all_gemini_contents[..num_cached] == request_contents[..num_cached]
                       && system_instruction_content == request_system_instruction // Ensure system prompt matches too
                    {
                        let new_contents = all_gemini_contents[num_cached..].to_vec();
                        if !new_contents.is_empty() {
                            bprintln!(dev: "Gemini Cache: Reusing cache '{}' ({} messages). Sending {} new messages.",
                                     existing_cache_name, num_cached, new_contents.len());
                            request_contents = new_contents;
                            request_system_instruction = None; // System prompt is in the cache
                            cached_content_name = Some(existing_cache_name.clone());
                            use_cache = true;
                        } else {
                             bprintln!(dev: "Gemini Cache: Request matches existing cache '{}' exactly, but sending full request to ensure response.", existing_cache_name);
                             // Invalidate cache as we are sending the full request now
                             *self.last_cache_info.lock().unwrap() = None;
                        }
                    } else {
                         bprintln!(dev: "Gemini Cache: Existing cache '{}' does not match current conversation prefix. Invalidating.", existing_cache_name);
                         // Invalidate cache if prefix doesn't match
                         *self.last_cache_info.lock().unwrap() = None;
                    }
                } else {
                     bprintln!(dev: "Gemini Cache: Current conversation shorter than or equal to existing cache '{}'. Invalidating.", existing_cache_name);
                     // Invalidate cache if conversation is now shorter or same length
                     *self.last_cache_info.lock().unwrap() = None;
                }
            }

            // If we didn't reuse a cache, check if we should create one
            // let mut cache_creation_tokens = 0; // Moved declaration outside the block
            if !use_cache {
                let total_input_tokens = self.count_tokens(&all_gemini_contents, system_instruction_content.as_ref()).await?;
                bprintln!(dev: "Gemini Cache: Estimated total input tokens: {}", total_input_tokens);

                if total_input_tokens > CACHE_TOKEN_THRESHOLD && all_gemini_contents.len() > 1 {
                    bprintln!(dev: "Gemini Cache: Token count {} exceeds threshold {}, attempting to create cache.", total_input_tokens, CACHE_TOKEN_THRESHOLD);

                    // Cache system prompt + all messages except the last one
                    let num_to_cache = all_gemini_contents.len() - 1;
                    let contents_to_cache = all_gemini_contents[..num_to_cache].to_vec();
                    let system_to_cache = system_instruction_content.clone();

                    match self.create_cached_content(contents_to_cache, system_to_cache).await {
                        Ok((new_cache_name, _num_cached, creation_usage)) => {
                            bprintln!(dev: "Gemini Cache: Successfully created cache: {}", new_cache_name);
                            cached_content_name = Some(new_cache_name);
                            cache_creation_tokens = creation_usage.cache_creation_input_tokens; // Store creation tokens
                            // Prepare request to use the cache: only send the last message
                            request_contents = vec![all_gemini_contents.last().unwrap().clone()];
                            request_system_instruction = None; // System prompt is in the cache
                            // Cache info (name, count) was already stored in create_cached_content
                        }
                        Err(e) => {
                            bprintln!(warn: "Gemini Cache: Failed to create CachedContent: {}. Proceeding without cache.", e);
                            // Invalidate cache info if creation failed
                             *self.last_cache_info.lock().unwrap() = None;
                            // Fallback: request_contents and request_system_instruction remain the full set
                        }
                    }
                } else {
                     bprintln!(dev: "Gemini Cache: Token count {} is below threshold {} or too few messages, skipping cache creation.", total_input_tokens, CACHE_TOKEN_THRESHOLD);
                     // Invalidate cache if below threshold
                     *self.last_cache_info.lock().unwrap() = None;
                     // Use full content, no cache name
                     cached_content_name = None;
                }
            }
            // --- Original Caching Logic End ---
        }

        // Check if we can reuse an existing cache
        if let Some((ref existing_cache_name, num_cached)) = *self.last_cache_info.lock().unwrap() {
            if num_cached < all_gemini_contents.len() {
                // Check if the cached part matches the current conversation start
                if all_gemini_contents[..num_cached] == request_contents[..num_cached]
                   && system_instruction_content == request_system_instruction // Ensure system prompt matches too
                {
                    let new_contents = all_gemini_contents[num_cached..].to_vec();
                    if !new_contents.is_empty() {
                        bprintln!(dev: "Gemini Cache: Reusing cache '{}' ({} messages). Sending {} new messages.",
                                 existing_cache_name, num_cached, new_contents.len());
                        request_contents = new_contents;
                        request_system_instruction = None; // System prompt is in the cache
                        cached_content_name = Some(existing_cache_name.clone());
                        use_cache = true;
                    } else {
                         bprintln!(dev: "Gemini Cache: Request matches existing cache '{}' exactly, but sending full request to ensure response.", existing_cache_name);
                         // Invalidate cache as we are sending the full request now
                         *self.last_cache_info.lock().unwrap() = None;
                    }
                } else {
                     bprintln!(dev: "Gemini Cache: Existing cache '{}' does not match current conversation prefix. Invalidating.", existing_cache_name);
                     // Invalidate cache if prefix doesn't match
                     *self.last_cache_info.lock().unwrap() = None;
                }
            } else {
                 bprintln!(dev: "Gemini Cache: Current conversation shorter than or equal to existing cache '{}'. Invalidating.", existing_cache_name);
                 // Invalidate cache if conversation is now shorter or same length
                 *self.last_cache_info.lock().unwrap() = None;
            }
        }

        // If we didn't reuse a cache, check if we should create one
        let mut cache_creation_tokens = 0; // Track tokens used for cache creation
        if !use_cache {
            let total_input_tokens = self.count_tokens(&all_gemini_contents, system_instruction_content.as_ref()).await?;
            bprintln!(dev: "Gemini Cache: Estimated total input tokens: {}", total_input_tokens);

            if total_input_tokens > CACHE_TOKEN_THRESHOLD && all_gemini_contents.len() > 1 {
                bprintln!(dev: "Gemini Cache: Token count {} exceeds threshold {}, attempting to create cache.", total_input_tokens, CACHE_TOKEN_THRESHOLD);

                // Cache system prompt + all messages except the last one
                let num_to_cache = all_gemini_contents.len() - 1;
                let contents_to_cache = all_gemini_contents[..num_to_cache].to_vec();
                let system_to_cache = system_instruction_content.clone();

                match self.create_cached_content(contents_to_cache, system_to_cache).await {
                    Ok((new_cache_name, _num_cached, creation_usage)) => {
                        bprintln!(dev: "Gemini Cache: Successfully created cache: {}", new_cache_name);
                        cached_content_name = Some(new_cache_name);
                        cache_creation_tokens = creation_usage.cache_creation_input_tokens; // Store creation tokens
                        // Prepare request to use the cache: only send the last message
                        request_contents = vec![all_gemini_contents.last().unwrap().clone()];
                        request_system_instruction = None; // System prompt is in the cache
                        // Cache info (name, count) was already stored in create_cached_content
                    }
                    Err(e) => {
                        bprintln!(warn: "Gemini Cache: Failed to create CachedContent: {}. Proceeding without cache.", e);
                        // Invalidate cache info if creation failed
                         *self.last_cache_info.lock().unwrap() = None;
                        // Fallback: request_contents and request_system_instruction remain the full set
                    }
                }
            } else {
                 bprintln!(dev: "Gemini Cache: Token count {} is below threshold {} or too few messages, skipping cache creation.", total_input_tokens, CACHE_TOKEN_THRESHOLD);
                 // Invalidate cache if below threshold
                 *self.last_cache_info.lock().unwrap() = None;
                 // Use full content, no cache name
                 cached_content_name = None;
            }
        }
        // --- End Caching Logic ---


        // Default max tokens if not provided - Increased to 8k
        let default_max_tokens = 8192; // Increased default
        let tokens = max_tokens.unwrap_or(default_max_tokens);

        let generation_config = GeminiGenerationConfig {
            max_output_tokens: Some(tokens as u32),
            temperature: Some(0.5), // Default temperature
            top_p: Some(0.95),      // Default top_p
            top_k: None,            // Default top_k
            stop_sequences: stop_sequences.map(|seqs| seqs.to_vec()).unwrap_or_default(),
        };

        // Prepare the final GeminiRequest
        let request = GeminiRequest {
            contents: request_contents, // Potentially modified contents
            system_instruction: request_system_instruction, // Potentially modified system instruction
            generation_config: Some(generation_config),
            safety_settings: None,
            cached_content: cached_content_name, // Add the cache name if using cache
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

        // Check for prompt feedback indicating a block
        if let Some(feedback) = &gemini_response.prompt_feedback {
            if let Some(reason) = &feedback.block_reason {
                // Prompt or response was blocked
                let error_msg = format!(
                    "Gemini API request blocked. Reason: {}. No candidates generated.",
                    reason
                );
                bprintln!(error: "{}", error_msg); // Log the block reason
                // Return an API error is clearest.
                return Err(LlmError::ApiError(error_msg));
            }
        }

        // Check if candidates list is empty (could happen even without explicit blockReason)
        if gemini_response.candidates.is_empty() {
            // Try to get finish_reason from promptFeedback if candidates are empty
            let finish_reason = gemini_response.prompt_feedback
                .as_ref()
                .and_then(|fb| fb.block_reason.as_deref())
                .unwrap_or("unknown_reason_empty_candidates");

            let error_msg = format!(
                "No candidates returned from Gemini API (reason: {}). This might be due to safety filters, recitation blocks, or an issue with the prompt.",
                finish_reason
            );
            bprintln!(warn:"{}", error_msg); // Log as warning

            // Return an API error, as no content was generated
            return Err(LlmError::ApiError(error_msg));
        }

        // Proceed with candidate processing since candidates exist
        let candidate = &gemini_response.candidates[0]; // Safe to index [0] because we checked is_empty
        let mut response_text = String::new();

        for part in &candidate.content.parts {
            if let Some(part_text) = &part.text {
                response_text.push_str(part_text);
            }
        }

        // Extract token usage, including cache usage
        let token_usage = TokenUsage {
            input_tokens: gemini_response
                .usage_metadata
                .prompt_token_count // Tokens in the non-cached part of the prompt
                .unwrap_or(0) as usize,
            output_tokens: gemini_response
                .usage_metadata
                .candidates_token_count // Tokens in the generated response
                .unwrap_or(0) as usize,
            cache_read_input_tokens: gemini_response
                .usage_metadata
                .cached_content_token_count // Tokens read from the cache
                .unwrap_or(0) as usize,
            // Use the value captured during cache creation, or 0 if no cache was created in this step
            cache_creation_input_tokens: cache_creation_tokens,
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
