//! Shared retry and timeout utilities for LLM backends
//!
//! This module provides standardized timeout and retry behavior for all LLM backends
//! including exponential backoff with jitter, configurable retry attempts, and
//! consistent error handling.

use crate::llm::LlmError;
use std::time::Duration;
use tokio::time::sleep;

/// Global configuration for all LLM API requests
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    
    /// Request timeout in seconds
    pub timeout_secs: u64,
    
    /// Whether to use exponential (true) or linear (false) backoff
    pub use_exponential: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 1000,  // 1 second initial delay
            max_delay_ms: 30000,  // Maximum 30 second delay (per TODO)
            timeout_secs: 180,    // 3 minute timeout (per TODO range of 100-200s)
            use_exponential: true, // Default to exponential backoff
        }
    }
}

/// Calculate exponential backoff delay with jitter
pub fn calculate_exponential_backoff(attempt: u32, config: &RetryConfig) -> u64 {
    if attempt == 0 {
        return 0; // No delay on first attempt
    }
    
    // Exponential backoff: delay = base * 2^(attempt-1)
    let exponent = attempt.saturating_sub(1) as u32;
    let exponential_delay = config.base_delay_ms * (2_u64.saturating_pow(exponent));
    
    // Add jitter (±10%) to prevent thundering herd problem
    let jitter_range = exponential_delay / 10; // 10% of delay
    let jitter = rand::random::<u64>() % (jitter_range * 2);
    let with_jitter = exponential_delay.saturating_add(jitter).saturating_sub(jitter_range);
    
    // Cap at maximum delay
    with_jitter.min(config.max_delay_ms)
}

/// Calculate linear backoff delay
pub fn calculate_linear_backoff(attempt: u32, config: &RetryConfig) -> u64 {
    if attempt == 0 {
        return 0; // No delay on first attempt
    }
    
    let linear_delay = config.base_delay_ms * (attempt as u64);
    
    // Add jitter (±10%) to prevent thundering herd problem
    let jitter_range = linear_delay / 10; // 10% of delay
    let jitter = rand::random::<u64>() % (jitter_range * 2);
    let with_jitter = linear_delay.saturating_add(jitter).saturating_sub(jitter_range);
    
    // Cap at maximum delay
    with_jitter.min(config.max_delay_ms)
}

/// Calculate backoff delay based on configuration
pub fn calculate_backoff_delay(attempt: u32, config: &RetryConfig) -> u64 {
    if config.use_exponential {
        calculate_exponential_backoff(attempt, config)
    } else {
        calculate_linear_backoff(attempt, config)
    }
}

/// Generic function to send API requests with retry logic
///
/// This function handles common retry patterns for all LLM APIs including:
/// - Timeout handling
/// - Rate limit (429) responses
/// - Server errors (5xx)
/// - Network errors
/// - Exponential or linear backoff with jitter
pub async fn send_api_request_with_retry<T, F>(
    _client: &reqwest::Client,
    _url: &str, 
    prepare_request: F,
    config: RetryConfig,
    provider_name: &str,
) -> Result<T, LlmError>
where
    T: serde::de::DeserializeOwned,
    F: Fn() -> reqwest::RequestBuilder,
{
    let mut attempts = 0;
    let timeout = Duration::from_secs(config.timeout_secs);

    loop {
        // Log the retry attempt if not the first attempt
        if attempts > 0 {
            bprintln!(warn: "🔄 Retry attempt {} of {} for {} API call", 
                     attempts, config.max_attempts, provider_name);
        }

        // Build the request with timeout
        let request = prepare_request()
            .timeout(timeout);
        
        // Send the request
        let response = request.send().await;

        match response {
            Ok(res) => {
                if res.status().is_success() {
                    return res.json::<T>().await.map_err(|e| {
                        LlmError::ApiError(format!("Failed to parse {} response: {}", provider_name, e))
                    });
                } else if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    // Handle rate limiting (429 Too Many Requests)
                    attempts += 1;
                    if attempts >= config.max_attempts {
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
                            // Use configured backoff strategy
                            let delay_ms = calculate_backoff_delay(attempts, &config);
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
                    if attempts >= config.max_attempts {
                        let status = res.status();
                        let error_text = res
                            .text()
                            .await
                            .unwrap_or_else(|_| "Unknown server error".to_string());
                            
                        return Err(LlmError::ApiError(format!(
                            "Max retries reached. {} server error {}: {}",
                            provider_name, status, error_text
                        )));
                    }
                    
                    // Calculate backoff delay
                    let delay_ms = calculate_backoff_delay(attempts, &config);
                    
                    bprintln!(error: "⚠️ {} API server error {}. Retrying in {} seconds (attempt {}/{})", 
                             provider_name, res.status(), delay_ms / 1000, attempts, config.max_attempts);
                    
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
                        "{} HTTP error {}: {}",
                        provider_name, status, error_text
                    )));
                }
            }
            Err(err) => {
                // Network-related errors (timeouts, connection issues)
                attempts += 1;
                
                if attempts >= config.max_attempts {
                    if err.is_timeout() {
                        return Err(LlmError::ApiError(format!(
                            "{} request timed out after {} seconds and {} retry attempts", 
                            provider_name,
                            config.timeout_secs,
                            config.max_attempts
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
                
                // Calculate backoff delay
                let delay_ms = calculate_backoff_delay(attempts, &config);
                
                if is_timeout {
                    bprintln!(warn: "⏱️ {} API request timed out after {} seconds. Retrying in {} seconds (attempt {}/{})",
                             provider_name, config.timeout_secs, delay_ms / 1000, attempts, config.max_attempts);
                } else {
                    bprintln!(warn: "🌐 Network error: {}. Retrying in {} seconds (attempt {}/{})",
                             err, delay_ms / 1000, attempts, config.max_attempts);
                }
                
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }
        }
    }
}