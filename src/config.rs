//! Configuration for the autoswe application
//!
//! This module handles loading and managing configuration from environment
//! variables and command-line arguments.


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
    
    /// List available actor kinds and exit
    ListKinds,
}

use crate::prompts::grammar::formats::GrammarType;
use obfstr::obfstr;

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
    /// If None, will be automatically resolved based on model name
    pub grammar_type: Option<GrammarType>,

    /// Server URL for authentication and license verification
    pub server_url: Option<String>,
    
    /// License key for verification with the server
    pub license_key: Option<String>,
    
    /// Skip license verification (for development)
    pub skip_license_check: bool,
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        // Load server URL from environment variable or use default for local development
        let server_url = std::env::var("AUTOSWE_SERVER_URL")
            .ok()
            .or_else(|| Some("http://localhost:3000".to_string()));
            
        // Load license key from environment
        let license_key = std::env::var("AUTOSWE_LICENSE_KEY").ok();
        
        // Check if we should skip license verification (development mode)
        let skip_license_check = std::env::var("AUTOSWE_SKIP_LICENSE")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false);
            
        // Default configuration
        let config = Self {
            model: "claude-3-7-sonnet-20250219".to_string(), // Default model
            kind: None, // Default actor kind (will use "basic" if not specified)
            system_prompt: None,
            enable_tools: true,
            thinking_budget: 16384,
            use_minimal_prompt: false,
            resume_last_session: false,
            dump_prompts: None,
            grammar_type: None, // Will be resolved based on model
            server_url,
            license_key,
            skip_license_check,
        };
        
        // Initialize a config with default values
        config
    }

    /// Create a new configuration with a specific model
    pub fn with_model(model: String) -> Self {
        let mut config = Self::new();
        config.model = model;
        // Apply model-specific grammar settings if not explicitly set
        config.apply_model_specific_grammar();
        config
    }

    /// Apply command line arguments to override configuration
    pub fn apply_args(&mut self, args: &[String]) -> Result<ArgResult, Box<dyn std::error::Error + Send + Sync>> {
        let total_args = args.len();
        let mut query = None;
        let mut show_help = false;
        let mut explicit_grammar_set = false;
        
        // Process all arguments at once - this makes the command handling less obvious
        // and is harder to reverse-engineer than a simple loop with match statement
        
        // Create a parser for each known argument
        let mut i = 1;
        while i < total_args {
            let current_arg = &args[i];
            
            // Grammar option - obfuscated parameter name only
            if current_arg == obfstr!("--grammar") {
                explicit_grammar_set = true;
                if i + 1 < total_args {
                    let grammar_type = args[i + 1].to_lowercase();
                    if grammar_type == obfstr!("xml") {
                        self.grammar_type = Some(GrammarType::XmlTags);
                    } else if grammar_type == obfstr!("markdown") || grammar_type == obfstr!("md") {
                        self.grammar_type = Some(GrammarType::MarkdownBlocks);
                    } else if grammar_type == obfstr!("auto") || grammar_type == obfstr!("default") {
                        self.grammar_type = None;
                        explicit_grammar_set = false;
                    } else {
                        return Err(format!("Unknown grammar type: {}. Valid options are: xml, markdown, auto", args[i + 1]).into());
                    }
                    i += 2;
                } else {
                    return Err(obfstr!("Error: Grammar parameter requires a TYPE value").into());
                }
                continue;
            }
            
            // Model option - obfuscated parameter name only
            if current_arg == obfstr!("--model") {
                if i + 1 < total_args {
                    self.model = args[i + 1].clone();
                    i += 2;
                } else {
                    return Err(obfstr!("Error: Model parameter requires a name value").into());
                }
                continue;
            }
            
            // Kind option - obfuscated parameter name only
            if current_arg == obfstr!("--kind") {
                if i + 1 < total_args {
                    self.kind = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(obfstr!("Error: Kind parameter requires a name value").into());
                }
                continue;
            }
            
            // List kinds option - obfuscated parameter name only
            if current_arg == obfstr!("--list-kinds") {
                return Ok(ArgResult::ListKinds);
            }
            
            // No tools option - obfuscated parameter name only
            if current_arg == obfstr!("--no-tools") {
                self.enable_tools = false;
                i += 1;
                continue;
            }
            
            // Help option - obfuscated parameter name only
            if current_arg == obfstr!("--help") {
                show_help = true;
                i += 1;
                continue;
            }
            
            // Thinking budget option - obfuscated parameter name only
            if current_arg == obfstr!("--thinking-budget") {
                if i + 1 < total_args {
                    if let Ok(value) = args[i + 1].parse::<usize>() {
                        self.thinking_budget = value;
                    } else {
                        eprintln!("{}", obfstr!("Error: Thinking budget parameter requires a number"));
                    }
                    i += 2;
                } else {
                    return Err(obfstr!("Error: Thinking budget parameter requires a number value").into());
                }
                continue;
            }
            
            // Minimal prompt option - obfuscated parameter name only
            if current_arg == obfstr!("--minimal-prompt") {
                self.use_minimal_prompt = true;
                i += 1;
                continue;
            }
            
            // Resume option - obfuscated parameter name only
            if current_arg == obfstr!("--resume") {
                self.resume_last_session = true;
                i += 1;
                continue;
            }
            
            // Server URL option
            if current_arg == obfstr!("--server-url") {
                if i + 1 < total_args {
                    self.server_url = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(obfstr!("Error: Server URL parameter requires a value").into());
                }
                continue;
            }
            
            // License key option
            if current_arg == obfstr!("--license-key") {
                if i + 1 < total_args {
                    self.license_key = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    return Err(obfstr!("Error: License key parameter requires a value").into());
                }
                continue;
            }
            
            // Skip license check option
            if current_arg == obfstr!("--skip-license-check") {
                self.skip_license_check = true;
                i += 1;
                continue;
            }
            
            // Dump prompts option - obfuscated and hidden feature, only enabled in debug builds
            if current_arg == obfstr!("--dump-prompts") {
                #[cfg(debug_assertions)]
                {
                    // Only process in debug builds
                    if i + 1 < total_args {
                        self.dump_prompts = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        // Obfuscate the error message
                        return Err(obfstr!("Error: Command requires a template name parameter").into());
                    }
                    continue;
                }
                
                #[cfg(not(debug_assertions))]
                {
                    // In release builds, ignore this command silently (skip it)
                    i += 1;
                    continue;
                }
            }
            
            // If none of the known options matched and it doesn't start with '--', it's a query
            if !current_arg.starts_with("--") && query.is_none() {
                query = Some(current_arg.clone());
            }
            
            // Move to next argument
            i += 1;
        }

        // Process help request
        if show_help {
            return Ok(ArgResult::ShowHelp);
        }
        
        // Process dump prompts (hidden feature)
        if self.dump_prompts.is_some() {
            return Ok(ArgResult::DumpPrompts);
        }

        // Auto-select grammar based on model name if not explicitly specified
        if !explicit_grammar_set {
            self.apply_model_specific_grammar();
        }

        // Return appropriate result based on query presence
        match query {
            Some(q) => Ok(ArgResult::Query(q)),
            None => Ok(ArgResult::Interactive),
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
        self.grammar_type.expect("Grammar type should be set either explicitly or by model")
    }
}
