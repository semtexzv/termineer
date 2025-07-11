//! LLM backend factory
//!
//! This module provides factory functions for creating LLM backends
//! based on model name inference.

use crate::config::Config;
use crate::llm::anthropic::Anthropic;
use crate::llm::cohere::CohereBackend;
use crate::llm::deepseek::DeepSeekBackend;
use crate::llm::grok::GrokBackend;
use crate::llm::openai::OpenAIBackend; // Import OpenAIBackend
use crate::llm::openrouter::OpenRouterBackend;
use crate::llm::{Backend, LlmError};
use std::env;

/// Supported model provider types
#[derive(Debug, PartialEq, Eq)]
pub enum Provider {
    /// Anthropic's Claude models
    Anthropic,
    /// OpenAI's models (Not implemented)
    OpenAI,
    /// Google's Gemini models
    Google,
    /// OpenRouter's unified API
    OpenRouter,
    /// DeepSeek's models
    DeepSeek,
    /// Cohere's models
    Cohere,
    /// xAI's Grok models
    Grok,
    /// Unknown provider
    Unknown(String),
}

/// Resolve OpenAI API key from environment variables
fn resolve_openai_api_key() -> Result<String, LlmError> {
    env::var("OPENAI_API_KEY")
        .map_err(|_| LlmError::ConfigError("OPENAI_API_KEY environment variable not set".into()))
}

/// Model information after parsing
struct ModelInfo {
    /// The provider to use
    provider: Provider,
    /// The actual model name to pass to the API
    model_name: String,
}

/// Create an LLM backend from configuration, inferring the provider from model name
pub fn create_backend(config: &Config) -> Result<Box<dyn Backend>, LlmError> {
    // Create the backend directly using the requested model
    // No model restrictions based on app mode - all users can access all models
    infer_backend_from_model(&config.model)
}

/// Parse a model string which may be in either format:
/// - "claude-3-opus-20240229" (provider inferred from model name)
/// - "anthropic/claude-3-opus-20240229" (explicit provider)
/// - "openrouter/openai/gpt-4o" (openrouter provider with model path)
/// - "deepseek/deepseek-chat" (deepseek provider with model name)
/// - "cohere/command-r" (cohere provider with model name)
fn parse_model_string(model_str: &str) -> ModelInfo {
    // Check if we have a provider/model format
    if let Some((provider, model)) = model_str.split_once('/') {
        // Check for OpenRouter format (openrouter/provider/model)
        if provider.trim().to_lowercase() == "openrouter" {
            return ModelInfo {
                provider: Provider::OpenRouter,
                model_name: model.trim().to_string(), // Keep the provider/model part intact
            };
        }

        // Extract provider and model for non-OpenRouter providers
        let provider_type = match provider.trim().to_lowercase().as_str() {
            "anthropic" => Provider::Anthropic,
            "openai" => Provider::OpenAI, // Handle explicit openai/ prefix
            "google" => Provider::Google,
            "deepseek" => Provider::DeepSeek,
            "cohere" => Provider::Cohere,
            "grok" | "xai" => Provider::Grok,
            other => Provider::Unknown(other.to_string()),
        };

        return ModelInfo {
            provider: provider_type,
            model_name: model.trim().to_string(),
        };
    }

    // No explicit provider, infer it from the model name
    let provider = if is_anthropic_model(model_str) {
        Provider::Anthropic
    } else if is_openai_model(model_str) {
        Provider::OpenAI
    } else if is_gemini_model(model_str) {
        Provider::Google
    } else if is_openrouter_model(model_str) {
        Provider::OpenRouter
    } else if is_deepseek_model(model_str) {
        Provider::DeepSeek
    } else if is_cohere_model(model_str) {
        Provider::Cohere
    } else if is_grok_model(model_str) {
        Provider::Grok
    } else {
        Provider::Unknown(String::new())
    };

    ModelInfo {
        provider,
        model_name: model_str.to_string(),
    }
}

/// Infer and create the appropriate backend based on model name
fn infer_backend_from_model(model_str: &str) -> Result<Box<dyn Backend>, LlmError> {
    let model_info = parse_model_string(model_str);

    match model_info.provider {
        Provider::Anthropic => {
            let api_key = resolve_anthropic_api_key()?;
            Ok(Box::new(Anthropic::new(api_key, model_info.model_name)))
        }
        Provider::OpenAI => { // Add OpenAI provider case
            let api_key = resolve_openai_api_key()?;
            Ok(Box::new(OpenAIBackend::new(api_key, model_info.model_name)))
        }
        Provider::Google => {
            let api_key = resolve_google_api_key()?;
            // Pass model name directly without translation
            Ok(Box::new(crate::llm::gemini::GeminiBackend::new(
                api_key,
                model_info.model_name,
            )))
        }
        Provider::DeepSeek => {
            let api_key = resolve_deepseek_api_key()?;
            Ok(Box::new(DeepSeekBackend::new(
                api_key,
                model_info.model_name,
            )))
        }
        Provider::Cohere => {
            let api_key = resolve_cohere_api_key()?;
            Ok(Box::new(CohereBackend::new(api_key, model_info.model_name)))
        }
        Provider::Grok => {
            let api_key = resolve_grok_api_key()?;
            Ok(Box::new(GrokBackend::new(api_key, model_info.model_name)))
        }
        Provider::OpenRouter => {
            let api_key = resolve_openrouter_api_key()?;

            // Get optional site URL and name for ranking on OpenRouter
            let site_url = env::var("OPENROUTER_SITE_URL").ok();
            let site_name = env::var("OPENROUTER_SITE_NAME").ok();

            Ok(Box::new(OpenRouterBackend::new(
                api_key,
                model_info.model_name,
                site_url,
                site_name,
            )))
        }
        Provider::OpenAI => Err(LlmError::ConfigError(
            "OpenAI provider is not implemented in this version".into(),
        )),
        Provider::Unknown(provider) => {
            let provider_msg = if provider.is_empty() {
                format!("Unknown model '{}'. Cannot determine provider.", model_str)
            } else {
                format!(
                    "Unknown provider '{}' specified in '{}'",
                    provider, model_str
                )
            };

            Err(LlmError::ConfigError(format!(
                "{}. Currently supporting these providers:\n\
                 - Anthropic models: 'claude-3-opus', 'claude-3-sonnet', etc.\n\
                 - Google models: 'gemini-1.5-pro', 'gemini-1.0-pro', etc.\n\
                 - OpenAI models: 'gpt-4o', 'gpt-4-turbo', 'gpt-3.5-turbo', etc.\n\
                 - DeepSeek models: 'deepseek-chat', 'deepseek-reasoner'\n\
                 - Cohere models: 'command-r', 'command-r-plus', 'command-light', etc.\n\
                 - Grok models: 'grok-2-1212', 'grok-beta'\n\
                 - OpenRouter: 'openrouter/openai/gpt-4o', 'openrouter/anthropic/claude-3-opus', etc.\n\
                 - Explicit provider format: 'openai/gpt-4o', 'anthropic/claude-3-opus', 'google/gemini-1.5-pro', 'grok/grok-2-1212'",
                provider_msg
            )))
        }
    }
}

/// Determine if a model name belongs to the Anthropic Claude family
/// Uses the model list from https://docs.anthropic.com/en/docs/about-claude/models/all-models
fn is_anthropic_model(model: &str) -> bool {
    model.starts_with("claude-")
}

/// Determine if a model name belongs to the OpenAI family
fn is_openai_model(model: &str) -> bool {
    model.starts_with("gpt-")
        || model.starts_with("text-")
        || model.starts_with("davinci")
        || model == "o1"
        || model.starts_with("o1-")
}

/// Determine if a model name belongs to the Google Gemini family
fn is_gemini_model(model: &str) -> bool {
    model.starts_with("gemini-")
}

/// Determine if a model name belongs to OpenRouter
fn is_openrouter_model(model: &str) -> bool {
    // OpenRouter-specific model identifiers
    model.starts_with("openrouter/") || model.starts_with("or-")
}

/// Determine if a model name belongs to DeepSeek
fn is_deepseek_model(model: &str) -> bool {
    // DeepSeek model identifiers
    model.starts_with("deepseek-") || model == "deepseek-chat" || model == "deepseek-reasoner"
}

/// Determine if a model name belongs to Cohere
fn is_cohere_model(model: &str) -> bool {
    // Cohere model identifiers
    model.starts_with("command-")
        || model == "command"
        || model == "command-r"
        || model == "command-r-plus"
        || model == "command-light"
}

/// Determine if a model name belongs to xAI Grok
fn is_grok_model(model: &str) -> bool {
    // Grok model identifiers
    model.starts_with("grok-") || model == "grok-2-1212" || model == "grok-beta"
}

/// Resolve Anthropic API key from environment variables
fn resolve_anthropic_api_key() -> Result<String, LlmError> {
    env::var("ANTHROPIC_API_KEY")
        .map_err(|_| LlmError::ConfigError("ANTHROPIC_API_KEY environment variable not set".into()))
}

/// Resolve Google API key from environment variables
fn resolve_google_api_key() -> Result<String, LlmError> {
    env::var("GOOGLE_API_KEY")
        .map_err(|_| LlmError::ConfigError("GOOGLE_API_KEY environment variable not set".into()))
}

/// Resolve OpenRouter API key from environment variables
fn resolve_openrouter_api_key() -> Result<String, LlmError> {
    env::var("OPENROUTER_API_KEY").map_err(|_| {
        LlmError::ConfigError("OPENROUTER_API_KEY environment variable not set".into())
    })
}

/// Resolve DeepSeek API key from environment variables
fn resolve_deepseek_api_key() -> Result<String, LlmError> {
    env::var("DEEPSEEK_API_KEY")
        .map_err(|_| LlmError::ConfigError("DEEPSEEK_API_KEY environment variable not set".into()))
}

/// Resolve Cohere API key from environment variables
fn resolve_cohere_api_key() -> Result<String, LlmError> {
    env::var("COHERE_API_KEY")
        .map_err(|_| LlmError::ConfigError("COHERE_API_KEY environment variable not set".into()))
}

/// Resolve Grok API key from environment variables
fn resolve_grok_api_key() -> Result<String, LlmError> {
    env::var("GROK_API_KEY")
        .map_err(|_| LlmError::ConfigError("GROK_API_KEY environment variable not set".into()))
}
