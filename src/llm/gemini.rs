//! Google Gemini API Provider
//!
//! Implementation of the LLM provider for Google's Gemini models.

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use crate::llm::{Backend, LlmResponse, LlmError, Message, Content, TokenUsage};

// Constants for the Gemini API
const API_URL_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

/// Gemini content part
#[derive(Serialize, Clone, Debug)]
struct GeminiContentPart {
    text: String,
}

/// Gemini request content
#[derive(Serialize, Clone, Debug)]
struct GeminiContent {
    parts: Vec<GeminiContentPart>,
    role: Option<String>,
}

/// Gemini generation configs
#[derive(Serialize, Clone, Debug)]
struct GenerationConfig {
    temperature: Option<f32>,
    max_output_tokens: Option<usize>,
    stop_sequences: Option<Vec<String>>,
}

/// Message request to the Gemini API
#[derive(Serialize, Clone, Debug)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    generation_config: Option<GenerationConfig>,
    system_instruction: Option<GeminiContent>,
}

/// Gemini API usage metrics
#[derive(Deserialize, Debug, Clone)]
struct GeminiUsage {
    prompt_token_count: usize,
    candidates_token_count: usize,
    total_token_count: usize,
}

/// Gemini response content part
#[derive(Deserialize, Debug, Clone)]
struct GeminiResponsePart {
    text: Option<String>,
}

/// Gemini candidate (response)
#[derive(Deserialize, Debug, Clone)]
struct GeminiCandidate {
    content: GeminiResponseContent,
    finish_reason: Option<String>,
    index: Option<usize>,
}

/// Gemini response content structure
#[derive(Deserialize, Debug, Clone)]
struct GeminiResponseContent {
    parts: Vec<GeminiResponsePart>,
    role: Option<String>,
}

/// Response from the Gemini API
#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    usage_metadata: Option<GeminiUsage>,
}

/// Implementation of LLM provider for Google Gemini
pub struct Gemini {
    /// API key for Google Gemini
    api_key: String,
    
    /// Model name to use
    model: String,
}

impl Gemini {
    /// Create a new Gemini provider with the specified API key and model
    pub fn new(api_key: String, model: String) -> Self {
        Self { 
            api_key, 
            model,
        }
    }
    
    /// Build the API URL for the specified model
    fn get_api_url(&self) -> String {
        format!("{}/{}:generateContent", API_URL_BASE, self.model)
    }
    
    /// Convert our Message type to Gemini format
    fn convert_message_to_gemini_content(&self, message: &Message) -> GeminiContent {
        let role = match message.role.as_str() {
            "user" => Some("user".to_string()),
            "assistant" => Some("model".to_string()),
            // No direct system role in Gemini, handled separately
            _ => None,
        };
        
        // Convert content to Gemini format
        let parts = match &message.content {
            Content::Text { text } => {
                vec![GeminiContentPart { text: text.clone() }]
            },
            // Handle other content types as needed
            _ => vec![], // Empty for unsupported types
        };
        
        GeminiContent {
            parts,
            role,
        }
    }
    
    /// Send a request to the Gemini API and return the raw JSON response
    fn send_api_request_raw(
        &self,
        request_json: serde_json::Value,
    ) -> Result<serde_json::Value, LlmError> {
        // Retry configuration
        let mut attempts = 0;
        let max_attempts = 3;
        let base_retry_delay_ms = 1000; // 1 second initial retry delay
        
        // Build the URL with API key
        let url = format!("{}?key={}", self.get_api_url(), self.api_key);

        loop {
            match ureq::post(&url)
                .set("Content-Type", "application/json")
                .send_json(request_json.clone())
            {
                Ok(res) => return Ok(res.into_json().map_err(|e| LlmError::ApiError(e.to_string()))?),
                Err(err) => match err {
                    ureq::Error::Status(429, response) => {
                        // Handle rate limiting (429 Too Many Requests)
                        attempts += 1;
                        if attempts >= max_attempts {
                            return Err(LlmError::ApiError(format!(
                                "Max retry attempts reached for rate limiting: {}",
                                response.status_text()
                            )));
                        }

                        // Try to get retry-after header, default to exponential backoff if not present
                        let retry_after = match response
                            .header("retry-after")
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

                        // Return a rate limit error with retry information
                        return Err(LlmError::RateLimitError { 
                            retry_after: Some(retry_after / 1000) 
                        });
                    }
                    ureq::Error::Status(status_code, response) => {
                        let error_message = response.into_string()
                            .unwrap_or_else(|_| "Unknown error".to_string());
                        
                        return Err(LlmError::ApiError(format!(
                            "HTTP error {}: {}",
                            status_code,
                            error_message
                        )));
                    }
                    // Pass through other errors
                    err => return Err(LlmError::ApiError(format!("HTTP request error: {}", err))),
                },
            }
        }
    }
    
    /// Send a request to the Gemini API with retry logic and deserialize to the given type
    fn send_api_request<T: serde::de::DeserializeOwned>(
        &self,
        request_json: serde_json::Value,
    ) -> Result<T, LlmError> {
        // Get the raw JSON response
        let json_response = self.send_api_request_raw(request_json)?;
        
        // Deserialize to the requested type
        serde_json::from_value(json_response)
            .map_err(|e| LlmError::ApiError(format!("Failed to deserialize response: {}", e)))
    }
}

impl Backend for Gemini {
    fn send_message(
        &self, 
        messages: &[Message], 
        system: Option<&str>,
        stop_sequences: Option<&[String]>,
        _thinking_budget: Option<usize>,
        _cache_points: Option<&BTreeSet<usize>>,
        max_tokens: Option<usize>,
    ) -> Result<LlmResponse, LlmError> {
        // Convert messages to Gemini format
        let mut gemini_contents: Vec<GeminiContent> = Vec::new();
        
        for message in messages {
            if message.role != "system" {
                gemini_contents.push(self.convert_message_to_gemini_content(message));
            }
        }
        
        // Handle system message - Gemini uses system_instruction
        let system_instruction = system.map(|s| {
            GeminiContent {
                parts: vec![GeminiContentPart { text: s.to_string() }],
                role: None,
            }
        });
        
        // Default max tokens if not provided
        let default_max_tokens = 8192; // Default max output size
        
        // Create the Gemini request
        let request = GeminiRequest {
            contents: gemini_contents,
            generation_config: Some(GenerationConfig {
                temperature: None, // Use default
                max_output_tokens: max_tokens.or(Some(default_max_tokens)), // Use provided or default
                stop_sequences: stop_sequences.map(|s| s.to_vec()),
            }),
            system_instruction,
        };
        
        // Convert to JSON and prepare for the API
        let json = serde_json::to_value(request)
            .map_err(|e| LlmError::ApiError(format!("Failed to serialize request: {}", e)))?;
        
        // Send the request
        let response_value = self.send_api_request_raw(json.clone())?;
        
        // Deserialize the response
        let response: GeminiResponse = serde_json::from_value(response_value)
            .map_err(|e| LlmError::ApiError(format!("Failed to deserialize response: {}", e)))?;
        
        // Check if we have candidates
        if response.candidates.is_empty() {
            return Err(LlmError::ApiError("No response candidates received".to_string()));
        }
        
        // Convert the first candidate to our format
        let candidate = &response.candidates[0];
        let content_text = candidate.content.parts.iter()
            .filter_map(|part| part.text.clone())
            .collect::<Vec<String>>()
            .join("");
        
        // Keep a clone of content_text for stop sequence detection later
        let content_text_for_detection = content_text.clone();
        
        // Convert to our content format
        let content = vec![Content::Text { text: content_text }];
        
        // Convert usage statistics
        let usage = response.usage_metadata.map(|u| TokenUsage {
            input_tokens: u.prompt_token_count,
            output_tokens: u.candidates_token_count,
            cache_creation_input_tokens: 0, // Not available in Gemini
            cache_read_input_tokens: 0, // Not available in Gemini
        });
        
        // Get finish reason from the candidate
        let stop_reason = candidate.finish_reason.clone();
        
        // Determine if a stop sequence was triggered
        // Gemini doesn't directly tell us which stop sequence was triggered,
        // so we need to infer it from the response and the stop sequences
        let stop_sequence = if stop_reason.as_deref() == Some("STOP_SEQUENCE") {
            // If stopped due to stop sequence, try to determine which one
            if let Some(sequences) = stop_sequences {
                // Find the first stop sequence that matches the end of the content_text
                sequences.iter()
                    .find(|&seq| content_text_for_detection.ends_with(seq))
                    .cloned()
            } else {
                None
            }
        } else {
            None
        };
        
        // Convert to LlmResponse with stop information
        Ok(LlmResponse {
            content,
            usage,
            stop_reason,
            stop_sequence,
        })
    }
    
    fn name(&self) -> &str {
        "google"
    }
    
    fn model(&self) -> &str {
        &self.model
    }
}