//! Configuration for the Termineer application
//!
//! This module handles loading and managing configuration values.

use crate::prompts::grammar::formats::GrammarType;
use lazy_static::lazy_static;
use std::fmt;
use std::sync::RwLock;

// Global application mode that can be accessed from anywhere
lazy_static! {
    pub static ref GLOBAL_APP_MODE: RwLock<AppMode> = RwLock::new(AppMode::Free);
}

/// Functions to get and set the global application mode
pub fn get_app_mode() -> AppMode {
    GLOBAL_APP_MODE.read().unwrap().clone()
}

pub fn set_app_mode(mode: AppMode) {
    let mut app_mode = GLOBAL_APP_MODE.write().unwrap();
    *app_mode = mode;
}

/// Application mode/tier that determines available features
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)]
pub enum AppMode {
    /// Free mode - limited features
    Free,
    /// Plus mode - intermediate features (reserved for future use)
    Plus,
    /// Pro mode - full feature set
    Pro,
}

impl fmt::Display for AppMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppMode::Free => write!(f, "Free"),
            AppMode::Plus => write!(f, "Plus"),
            AppMode::Pro => write!(f, "Pro"),
        }
    }
}

// Only use obfuscated versions for command-line arguments
// No plain text versions in the code

/// Application configuration structure
#[derive(Clone)]
pub struct Config {
    /// Model name to use (will infer provider from this)
    pub model: String,

    /// Kind of agent to use (basic, minimal, researcher, etc.)
    pub kind: Option<String>,

    /// Custom system prompt (generated from template or directly provided)
    pub system_prompt: Option<String>,

    /// Whether to enable tools
    pub enable_tools: bool,

    /// List of specific tools to disable by name
    pub disabled_tools: Vec<String>,

    /// Budget for "thinking" capabilities
    pub thinking_budget: usize,

    /// Maximum tokens to generate in the response (None = use model default)
    pub max_token_output: Option<usize>,

    /// Whether to use a minimal system prompt
    pub use_minimal_prompt: bool,

    #[cfg(debug_assertions)]
    /// Whether to dump prompts and exit
    pub dump_prompts: Option<String>,

    /// Grammar type for tool formatting
    /// If None, will be automatically resolved based on model name
    pub grammar_type: Option<GrammarType>,

    /// User email after authentication
    pub user_email: Option<String>,

    /// User subscription type
    pub subscription_type: Option<String>,

    /// Skip authentication (for development)
    pub skip_auth: bool,
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        // Default configuration
        Self {
            model: "claude-3-7-sonnet-20250219".to_string(), // Default model
            kind: None, // Default actor kind (will use "basic" if not specified)
            system_prompt: None,
            enable_tools: true,
            disabled_tools: Vec::new(), // No tools disabled by default
            thinking_budget: 8192,
            max_token_output: None, // No limit by default, use model's default
            use_minimal_prompt: false,
            #[cfg(debug_assertions)]
            dump_prompts: None,
            grammar_type: None, // Will be resolved based on model
            user_email: None,
            subscription_type: None,
            skip_auth: false,
        }
    }

    /// Apply model-specific settings, such as selecting the appropriate grammar
    /// based on the model name
    ///
    /// This method is only applied when grammar_type is None (no explicit grammar set)
    pub fn apply_model_specific_grammar(&mut self) {
        // Only apply if grammar_type is None (not explicitly set)
        if self.grammar_type.is_some() {
            return;
        }

        // Use model name to determine appropriate grammar type
        let model_lower = self.model.to_lowercase();

        // Set the grammar based on model name patterns
        if model_lower.contains("gemini") {
            // Google's Gemini models work better with Markdown grammar
            self.grammar_type = Some(GrammarType::MarkdownBlocks);
        } else if model_lower.contains("gpt") || model_lower.contains("o1") {
            // GPT-4 and newer OpenAI models tend to work well with XML tags
            self.grammar_type = Some(GrammarType::XmlTags);
        } else if model_lower.contains("mistral") || model_lower.contains("mixtral") {
            // Mistral models support both, but XML may be more precise
            self.grammar_type = Some(GrammarType::XmlTags);
        } else if model_lower.contains("llama") || model_lower.contains("meta") {
            // Meta's Llama models, choose based on your testing
            self.grammar_type = Some(GrammarType::XmlTags);
        } else {
            // Default for Claude and other models: XML tags
            self.grammar_type = Some(GrammarType::XmlTags);
        }
    }

    /// Get the resolved grammar type, applying model-specific resolution if needed
    pub fn get_grammar_type(&mut self) -> GrammarType {
        // Apply model-specific grammar if not already set
        if self.grammar_type.is_none() {
            self.apply_model_specific_grammar();
        }

        // Return the grammar type (should be Some after apply_model_specific_grammar)
        // If it's still None after apply_model_specific_grammar (which shouldn't happen),
        // we'll panic to make the issue obvious rather than silently defaulting
        self.grammar_type
            .expect("Grammar type should be set either explicitly or by model")
    }
}
