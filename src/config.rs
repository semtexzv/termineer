//! Configuration for the autoswe application
//!
//! This module handles loading and managing configuration from environment
//! variables and command-line arguments.

use std::env;
use std::error::Error;

// Return type for apply_args to differentiate between different run modes
#[derive(Debug)]
pub enum ArgResult {
    /// Query provided (non-interactive mode)
    Query(String),

    /// No query (interactive mode)
    Interactive,

    /// Help requested
    ShowHelp,
    
    /// Dump prompts and exit
    DumpPrompts,
}

use crate::prompts::grammar::formats::GrammarType;

/// Application configuration structure
#[derive(Clone)]
pub struct Config {
    /// Model name to use (will infer provider from this)
    pub model: String,

    /// Template to use for the system prompt (basic, minimal, researcher, etc.)
    pub template_name: Option<String>,
    
    /// Custom system prompt (generated from template or directly provided)
    /// This is kept for backward compatibility
    pub system_prompt: Option<String>,

    /// Whether to enable tools
    pub enable_tools: bool,

    /// Budget for "thinking" capabilities
    pub thinking_budget: usize,

    /// Whether to use a minimal system prompt
    pub use_minimal_prompt: bool,

    /// Whether to resume the last session
    pub resume_last_session: bool,
    
    /// Whether to dump prompts and exit
    pub dump_prompts: Option<String>,
    
    /// Grammar type for tool formatting
    pub grammar_type: GrammarType,
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        // Default configuration
        Self {
            model: "claude-3-7-sonnet-20250219".to_string(), // Default model
            template_name: None,
            system_prompt: None,
            enable_tools: true,
            thinking_budget: 16384,
            use_minimal_prompt: false,
            resume_last_session: false,
            dump_prompts: None,
            grammar_type: GrammarType::XmlTags, // Default to XML tags for compatibility
        }
    }

    /// Create a new configuration with a specific model
    pub fn with_model(model: String) -> Self {
        let mut config = Self::new();
        config.model = model;
        // Apply model-specific grammar settings
        config.apply_model_specific_grammar();
        config
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        // Create default configuration
        let mut config = Self::new();

        // Check for provider-specific env vars for basic validation
        // This will be handled in detail by each provider, but we do a basic check here
        if env::var("ANTHROPIC_API_KEY").is_err() {
            eprintln!("Warning: ANTHROPIC_API_KEY environment variable not set");
            // We don't return an error here since the provider will handle this
        }
        
        if env::var("OPENROUTER_API_KEY").is_err() {
            eprintln!("Warning: OPENROUTER_API_KEY environment variable not set");
            // We don't return an error here since the provider will handle this
        }

        // Override model from environment if provided
        if let Ok(model) = env::var("MODEL") {
            config.model = model;
            // Apply model-specific grammar settings
            config.apply_model_specific_grammar();
        }

        Ok(config)
    }

    /// Apply command line arguments to override configuration
    pub fn apply_args(&mut self, args: &[String]) -> Result<ArgResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut i = 1;
        let mut query = None;
        let mut show_help = false;

        while i < args.len() {
            match args[i].as_str() {
                "--grammar" => {
                    if i + 1 < args.len() {
                        match args[i + 1].to_lowercase().as_str() {
                            "xml" => self.grammar_type = GrammarType::XmlTags,
                            "markdown" | "md" => self.grammar_type = GrammarType::MarkdownBlocks,
                            _ => return Err(format!("Unknown grammar type: {}. Valid options are: xml, markdown", args[i + 1]).into()),
                        }
                        i += 2;
                    } else {
                        return Err("Error: --grammar requires a TYPE (xml, markdown)".into());
                    }
                }
                "--model" => {
                    if i + 1 < args.len() {
                        self.model = args[i + 1].clone();
                        i += 2;
                    } else {
                        return Err("Error: --model requires a MODEL_NAME".into());
                    }
                }
                "--system" => {
                    if i + 1 < args.len() {
                        self.template_name = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err("Error: --system requires a TEMPLATE_NAME (basic, minimal, researcher)".into());
                    }
                }
                "--no-tools" => {
                    self.enable_tools = false;
                    i += 1;
                }
                "--help" => {
                    show_help = true;
                    i += 1;
                }
                "--thinking-budget" => {
                    if i + 1 < args.len() {
                        if let Ok(value) = args[i + 1].parse::<usize>() {
                            self.thinking_budget = value;
                        } else {
                            eprintln!("Error: --thinking-budget requires a number");
                        }
                        i += 2;
                    } else {
                        return Err("Error: --thinking-budget requires a NUMBER".into());
                    }
                }
                "--minimal-prompt" => {
                    self.use_minimal_prompt = true;
                    i += 1;
                }
                "--resume" => {
                    self.resume_last_session = true;
                    i += 1;
                }
                "--dump-prompts" => {
                    if i + 1 < args.len() {
                        self.dump_prompts = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err("Error: --dump-prompts requires a TEMPLATE_NAME (basic/minimal)".into());
                    }
                }
                _ => {
                    // If it doesn't start with --, treat it as the query
                    if !args[i].starts_with("--") && query.is_none() {
                        query = Some(args[i].clone());
                    }
                    i += 1;
                }
            }
        }

        if show_help {
            return Ok(ArgResult::ShowHelp);
        }
        
        if self.dump_prompts.is_some() {
            return Ok(ArgResult::DumpPrompts);
        }

        // Auto-select grammar based on model name if user didn't explicitly specify a grammar
        // This ensures that even when model is changed via CLI, the grammar is updated accordingly
        if !args.iter().any(|arg| arg == "--grammar") {
            self.apply_model_specific_grammar();
        }

        match query {
            Some(q) => Ok(ArgResult::Query(q)),
            None => Ok(ArgResult::Interactive),
        }
    }
    
    /// Apply model-specific settings, such as selecting the appropriate grammar
    /// based on the model name
    fn apply_model_specific_grammar(&mut self) {
        // Use model name to determine grammar type
        let model_lower = self.model.to_lowercase();
        
        // Gemini models work better with Markdown grammar
        if model_lower.contains("gemini") {
            self.grammar_type = crate::prompts::grammar::formats::GrammarType::MarkdownBlocks;
        } else {
            // XML tags for all other models including Claude
            self.grammar_type = crate::prompts::grammar::formats::GrammarType::XmlTags;
        }
    }
}
