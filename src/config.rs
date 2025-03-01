//! Configuration for the autoswe application
//!
//! This module handles loading and managing configuration from environment
//! variables and command-line arguments.

use std::env;
use std::error::Error;

// Return type for apply_args to differentiate between help and interactive mode
#[derive(Debug)]
pub enum ArgResult {
    /// Query provided (non-interactive mode)
    Query(String),
    
    /// No query (interactive mode)
    Interactive,
    
    /// Help requested
    ShowHelp,
}

/// Application configuration structure
#[derive(Clone)]
pub struct Config {
    /// Model name to use (will infer provider from this)
    pub model: String,
    
    /// Custom system prompt
    pub system_prompt: Option<String>,
    
    /// Whether to enable tools
    pub enable_tools: bool,
    
    /// Budget for "thinking" capabilities
    pub thinking_budget: usize,
    
    /// Whether to use a minimal system prompt
    pub use_minimal_prompt: bool,
    
    /// Whether to resume the last session
    pub resume_last_session: bool,
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        // Default configuration
        Self {
            model: "claude-3-7-sonnet-20250219".to_string(), // Default model
            system_prompt: None,
            enable_tools: true,
            thinking_budget: 8192,
            use_minimal_prompt: false,
            resume_last_session: false,
        }
    }
    
    /// Create a new configuration with a specific model
    pub fn with_model(model: String) -> Self {
        let mut config = Self::new();
        config.model = model;
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
        
        // Override model from environment if provided
        if let Ok(model) = env::var("AUTOSWE_MODEL") {
            config.model = model;
        }
        
        // Override thinking budget from environment if provided
        if let Ok(budget) = env::var("AUTOSWE_THINKING_BUDGET") {
            if let Ok(budget) = budget.parse::<usize>() {
                config.thinking_budget = budget;
            }
        }
        
        // Override tools enabled from environment if provided
        if let Ok(tools) = env::var("AUTOSWE_ENABLE_TOOLS") {
            config.enable_tools = tools.to_lowercase() == "true" || tools == "1";
        }

        Ok(config)
    }

    /// Apply command line arguments to override configuration
    pub fn apply_args(&mut self, args: &[String]) -> Result<ArgResult, Box<dyn Error>> {
        let mut i = 1;
        let mut query = None;
        let mut show_help = false;

        while i < args.len() {
            match args[i].as_str() {
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
                        self.system_prompt = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        return Err("Error: --system requires a PROMPT".into());
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

        match query {
            Some(q) => Ok(ArgResult::Query(q)),
            None => Ok(ArgResult::Interactive)
        }
    }
}