//! Shared retry and timeout utilities for LLM backends
//!
//! This module provides standardized timeout and retry behavior for all LLM backends
//! including:
//! - Linear backoff with jitter by default (supports exponential as an option)
//! - Long timeouts (180 seconds, within 100-200 seconds range) for potentially slow API responses
//! - Maximum waiting time of 30 seconds between retries
//! - Consistent error handling with network error and timeout detection using library timeout detection
//! - Helper functions to create standardized retry configurations

use crate::llm::LlmError;
use std::time::Duration;
use tokio::time::sleep;

/// Standard timeout and retry constants for LLM APIs
pub mod constants {
    /// Minimum recommended timeout for LLM API calls (100 seconds)
    pub const MIN_RECOMMENDED_TIMEOUT_SECS: u64 = 100;

    /// Maximum recommended timeout for LLM API calls (200 seconds)
    pub const MAX_RECOMMENDED_TIMEOUT_SECS: u64 = 200;

    /// Default timeout for LLM API calls (180 seconds)
    pub const DEFAULT_TIMEOUT_SECS: u64 = 180;

    /// Maximum waiting time between retries (30 seconds)
    pub const MAX_RETRY_DELAY_MS: u64 = 30000;

    /// Default base delay for linear backoff (1 second)
    pub const DEFAULT_BASE_DELAY_MS: u64 = 1000;

    /// Default maximum retry attempts
    pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
}

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
        // Use the standard configuration by default
        // (180s timeout, linear backoff, 30s max delay)
        create_standard_retry_config()
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
    let with_jitter = exponential_delay
        .saturating_add(jitter)
        .saturating_sub(jitter_range);

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
    let with_jitter = linear_delay
        .saturating_add(jitter)
        .saturating_sub(jitter_range);

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

/// Creates a standard retry configuration with linear backoff
///
/// This is the recommended configuration for LLM API calls:
/// - Linear backoff: Each retry increases delay by a fixed amount
/// - Maximum wait time of 30 seconds between retries (constants::MAX_RETRY_DELAY_MS)
/// - Long timeout (180 seconds) for potentially slow API responses
///
/// # Returns
///
/// A `RetryConfig` with standard settings for LLM API calls
pub fn create_standard_retry_config() -> RetryConfig {
    RetryConfig {
        max_attempts: constants::DEFAULT_MAX_ATTEMPTS,
        base_delay_ms: constants::DEFAULT_BASE_DELAY_MS,
        max_delay_ms: constants::MAX_RETRY_DELAY_MS, // 30 seconds
        timeout_secs: constants::DEFAULT_TIMEOUT_SECS, // 180 seconds
        use_exponential: false,                      // Use linear backoff
    }
}

/// Creates a custom retry configuration with timeout validation
///
/// This function ensures that the timeout value is within recommended
/// range (100-200 seconds) and logs a warning if it's not.
///
/// # Parameters
///
/// * `max_attempts` - Maximum number of retry attempts
/// * `base_delay_ms` - Base delay between retries in milliseconds
/// * `max_delay_ms` - Maximum delay between retries in milliseconds (recommended: 30000ms)
/// * `timeout_secs` - Request timeout in seconds (recommended: 100-200s)
/// * `use_exponential` - Whether to use exponential (true) or linear (false) backoff (recommended: false)
///
/// # Returns
///
/// A `RetryConfig` with the specified settings
pub fn create_custom_retry_config(
    max_attempts: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
    timeout_secs: u64,
    use_exponential: bool,
) -> RetryConfig {
    // Validate timeout is within recommended range
    if timeout_secs < constants::MIN_RECOMMENDED_TIMEOUT_SECS {
        bprintln!(warn: "⚠️ Timeout of {} seconds is below the recommended minimum of {} seconds for LLM API calls",
                 timeout_secs, constants::MIN_RECOMMENDED_TIMEOUT_SECS);
    } else if timeout_secs > constants::MAX_RECOMMENDED_TIMEOUT_SECS {
        bprintln!(warn: "⚠️ Timeout of {} seconds exceeds the recommended maximum of {} seconds for LLM API calls",
                 timeout_secs, constants::MAX_RECOMMENDED_TIMEOUT_SECS);
    }

    RetryConfig {
        max_attempts,
        base_delay_ms,
        max_delay_ms,
        timeout_secs,
        use_exponential,
    }
}

/// Creates a retry configuration with linear backoff strategy
///
/// This is specifically optimized for LLM API calls:
/// - Linear backoff: Each retry increases delay by a fixed amount + jitter
/// - Long timeout (180 seconds) for potentially slow API responses
/// - Maximum wait time of 30 seconds between retries
///
/// # Parameters
///
/// * `max_attempts` - Maximum number of retry attempts (default: 5)
/// * `base_delay_ms` - Base delay between retries in milliseconds (default: 1000ms)
///
/// # Returns
///
/// A `RetryConfig` with linear backoff strategy
pub fn create_linear_backoff_config(
    max_attempts: Option<u32>,
    base_delay_ms: Option<u64>,
) -> RetryConfig {
    RetryConfig {
        max_attempts: max_attempts.unwrap_or(constants::DEFAULT_MAX_ATTEMPTS),
        base_delay_ms: base_delay_ms.unwrap_or(constants::DEFAULT_BASE_DELAY_MS),
        max_delay_ms: constants::MAX_RETRY_DELAY_MS, // 30 seconds max
        timeout_secs: constants::DEFAULT_TIMEOUT_SECS, // 180 seconds
        use_exponential: false,                      // Use linear backoff
    }
}

/// Generic function to send API requests with retry logic
///
/// This function handles common retry patterns for all LLM APIs including:
/// - Long-running request timeout handling (default 180 seconds)
/// - Rate limit (429) responses with respect for retry-after headers
/// - Server errors (5xx) with linear backoff by default
/// - Network errors with automatic retry
/// - Linear backoff with jitter (configurable to exponential if needed)
/// - Library-level timeout detection for network issues
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
        let request = prepare_request().timeout(timeout);

        // Send the request
        let response = request.send().await;

        match response {
            Ok(res) => {
                // Log the HTTP status code received
                bprintln!(dev: "{} API response status: {}", provider_name, res.status());

                if res.status().is_success() {
                    // Read the response body first to allow logging it on parsing failure
                    let response_body = match res.text().await {
                        Ok(body) => body,
                        Err(e) => {
                            // Error reading the response body itself
                            return Err(LlmError::ApiError(format!(
                                "Failed to read {} response body: {}",
                                provider_name, e
                            )));
                        }
                    };

                    // Attempt to parse the captured body
                    return serde_json::from_str::<T>(&response_body).map_err(|e| {
                        // Log the raw body along with the parsing error
                        bprintln!(error: "Failed to parse {} response. Error: {}. Body:\n{}", provider_name, e, response_body);
                        LlmError::ApiError(format!(
                            "Failed to parse {} response: {}",
                            provider_name, e
                        ))
                        // Consider including a truncated body in the error message itself if needed,
                        // but logging it might be sufficient. Example:
                        // let truncated_body = response_body.chars().take(500).collect::<String>();
                        // LlmError::ApiError(format!(
                        //     "Failed to parse {} response: {}. Body (truncated): {}",
                        //     provider_name, e, truncated_body
                        // ))
                    });
                } else if res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    // Handle rate limiting (429 Too Many Requests)
                    attempts += 1;
                    if attempts >= config.max_attempts {
                        return Err(LlmError::RateLimitError { retry_after: None });
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
                            bprintln!(
                                "⏱️ Rate limit exceeded. Server requested retry after {} seconds",
                                value
                            );
                            delay_ms
                        }
                        None => {
                            // Use configured backoff strategy
                            let delay_ms = calculate_backoff_delay(attempts, &config);
                            bprintln!(
                                "⏱️ Rate limit exceeded. Retrying in {} seconds",
                                delay_ms / 1000
                            );
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
                            provider_name, config.timeout_secs, config.max_attempts
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
