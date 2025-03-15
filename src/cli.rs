//! Command-line interface definition and argument parsing
//!
//! This module uses clap to define and parse command-line arguments.

use clap::{Parser, Subcommand};
use crate::prompts::grammar::formats::GrammarType;

/// Command-line arguments for Termineer
#[derive(Parser, Debug)]
#[command(
    name = "Termineer",
    about = "Your Terminal Engineer - an AI assistant in your terminal",
    version,
    author,
    long_about = "Termineer provides an interactive AI assistant in your terminal with tool capabilities for enhanced productivity."
)]
pub struct Cli {
    /// The query to process in non-interactive mode
    pub query: Option<String>,

    /// The model to use for the AI assistant
    #[arg(long, default_value = "claude-3-7-sonnet-20250219")]
    pub model: String,

    /// The agent kind/template to use
    #[arg(long)]
    pub kind: Option<String>,

    /// Disable tools
    #[arg(long)]
    pub no_tools: bool,

    /// The thinking budget in tokens
    #[arg(long, default_value_t = 8192)]
    pub thinking_budget: usize,

    /// Use minimal prompt
    #[arg(long)]
    pub minimal_prompt: bool,

    /// Resume last session
    #[arg(long)]
    pub resume: bool,

    /// Grammar type to use (xml, markdown, auto)
    #[arg(long, value_parser = parse_grammar_type)]
    pub grammar: Option<GrammarType>,

    /// Skip authentication (for development)
    #[arg(long, hide = true)]
    pub skip_auth: bool,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Subcommands for Termineer
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Login to Termineer account
    Login,

    /// List available agent kinds/templates
    ListKinds,

    /// Run a workflow from the .termineer/workflows directory
    Workflow {
        /// Name of the workflow to run
        name: Option<String>,
        
        /// Parameters for the workflow in key=value format
        #[arg(long = "param", short = 'p')]
        parameters: Vec<String>,
        
        /// Additional query to pass to the workflow (everything after parameters)
        #[arg(trailing_var_arg = true)]
        query: Vec<String>,
    },

    /// Dump prompt templates (hidden, debug-only feature)
    #[cfg(debug_assertions)]
    #[command(hide = true)]
    DumpPrompts {
        /// Template name to dump
        template: String,
    },
}

/// Parse grammar type from string
fn parse_grammar_type(arg: &str) -> Result<GrammarType, String> {
    match arg.to_lowercase().as_str() {
        "xml" => Ok(GrammarType::XmlTags),
        "markdown" | "md" => Ok(GrammarType::MarkdownBlocks),
        "auto" | "default" => Err("Use no argument for auto grammar selection".to_string()),
        _ => Err(format!("Unknown grammar type: {}. Valid options: xml, markdown", arg)),
    }
}

/// Convert the Cli struct to the application's Config
pub fn cli_to_config(cli: &Cli) -> crate::config::Config {
    let mut config = crate::config::Config::new();
    
    // Basic options
    config.model = cli.model.clone();
    config.kind = cli.kind.clone();
    config.enable_tools = !cli.no_tools;
    config.thinking_budget = cli.thinking_budget;
    config.use_minimal_prompt = cli.minimal_prompt;
    config.resume_last_session = cli.resume;
    config.grammar_type = cli.grammar;
    config.skip_auth = cli.skip_auth;
    
    // Special commands
    #[cfg(debug_assertions)]
    if let Some(Commands::DumpPrompts { template }) = &cli.command {
        config.dump_prompts = Some(template.clone());
    }
    
    // Apply model-specific grammar if not explicitly set
    if config.grammar_type.is_none() {
        config.apply_model_specific_grammar();
    }
    
    config
}