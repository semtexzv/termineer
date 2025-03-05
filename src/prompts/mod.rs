//! Prompt template system
//!
//! This module handles loading, parsing, and rendering prompt templates.
//! It uses a Handlebars template system for flexible prompt composition.

pub mod grammar;
pub mod handlebars;

// Protected prompts module for encrypted templates
pub mod protected;

use std::sync::Arc;
use anyhow::bail;
pub use grammar::{Grammar, XmlGrammar};
pub use handlebars::TemplateManager;

// Include the generated list of available kinds from build script
include!(concat!(env!("OUT_DIR"), "/encrypted_prompts.rs"));

/// List of all available tools
pub const ALL_TOOLS: &[&str] = &[
    "shell", "read", "write", "patch", "fetch", "search", "mcp",
    "task", "done", "agent", "wait"
];

/// List of read-only tools (excludes tools that can modify the filesystem)
pub const READONLY_TOOLS: &[&str] = &[
    "shell", "read", "fetch", "search", "mcp", "done", "agent", "wait"
];

/// Check if a kind name is available in the compiled templates
pub fn is_valid_kind(kind_name: &str) -> bool {
    AVAILABLE_KINDS_ARRAY.iter().position(|it| it == &kind_name).is_some()
}

/// Get suggestions for kinds based on partial match
/// 
/// This function returns up to 3 suggestions that contain the query string.
/// 
/// # Arguments
/// * `query` - The partial kind name to match
/// 
/// # Returns
/// Vector of kind name suggestions (empty if no matches)
pub fn get_kind_suggestions(query: &str) -> Vec<String> {
    let query = query.to_lowercase();
    let mut matches: Vec<String> = AVAILABLE_KINDS_ARRAY
        .iter()
        .filter(|kind| kind.to_lowercase().contains(&query))
        .cloned()
        .collect();
    
    // Limit to 3 suggestions
    if matches.len() > 3 {
        matches.truncate(3);
    }
    
    matches
}

/// Render a template with specific tools enabled
///
/// This function renders a template with the specified tools enabled.
/// It handles loading the template and partials.
///
/// # Arguments
/// * `template_name` - The name of the template file (without .hbs extension)
/// * `enabled_tools` - The list of tools to enable in the template
///
/// # Returns
/// The rendered template as a string, or an error message
pub fn render_template(template_name: &str, enabled_tools: &[&str], grammar: Arc<dyn Grammar>) -> anyhow::Result<String> {
    
    // Create a template manager
    let mut template_manager = TemplateManager::new(grammar);
    
    // Load all templates to ensure partials are available
    match template_manager.load_all_templates() {
        Ok(_) => {
            // Render the template with specified tools enabled
            match template_manager.render_with_tool_enablement(template_name, enabled_tools) {
                Ok(rendered) => Ok(rendered),
                Err(e) => {
                    bail!("Error generating system prompt: {}", template_name);
                },
            }
        },
        Err(e) => bail!("Error loading templates: {}", e),
    }
}

/// Generate a system prompt with appropriate tool documentation
///
/// # Arguments
/// * `enabled_tools` - The list of tool names to enable
/// * `use_minimal` - Whether to use the minimal prompt template (legacy behavior)
/// * `kind_name` - Optional specific template to use (overrides use_minimal)
/// * `grammar` - The grammar implementation to use for formatting
///
/// # Returns
/// The generated system prompt as a string, or an error with suggestions
pub fn generate_system_prompt(
    enabled_tools: &[&str], 
    use_minimal: bool, 
    kind_name: Option<&str>, 
    grammar: Arc<dyn Grammar>
) -> Result<String, anyhow::Error> {
    // Determine which template to use
    let kind = if let Some(name) = kind_name {
        // If a specific template is provided, validate it
        if !is_valid_kind(name) {
            // Get suggestions for similar kinds
            let suggestions = get_kind_suggestions(name);
            
            // Create helpful error message with suggestions
            let mut error_msg = format!("Invalid agent kind: '{}'", name);
            if !suggestions.is_empty() {
                error_msg.push_str("\n\nDid you mean one of these?");
                for suggestion in suggestions {
                    error_msg.push_str(&format!("\n  - {}", suggestion));
                }
            }
            
            // Return the error with suggestions
            bail!(error_msg);
        }
        
        // Kind is valid, use it
        name
    } else if use_minimal {
        // Legacy behavior: if minimal is requested, use minimal template
        "minimal"
    } else {
        // Default to basic template
        "general"
    };
    
    let kind = format!("kind/{}", kind);
    
    // Render the template
    match render_template(&kind, enabled_tools, grammar) {
        Ok(prompt) => Ok(prompt),
        Err(e) => {
            bail!("Failed to render template '{}': {}", kind, e);
        }
    }
}

/// Select a grammar implementation based on model name
///
/// This function returns the appropriate grammar implementation
/// for the specified model. It checks the model name against known
/// patterns to determine the most compatible grammar format.
///
/// # Arguments
/// * `model_name` - The name of the model to select grammar for
///
/// # Returns
/// A boxed Grammar implementation appropriate for the model
pub fn select_grammar_for_model(model_name: &str) -> Arc<dyn Grammar> {
    use grammar::formats::{get_grammar, GrammarType};
    
    // Choose grammar based on model name
    let model_lower = model_name.to_lowercase();
    
    if model_lower.contains("gemini") {
        // Use markdown grammar for Gemini models
        get_grammar(GrammarType::MarkdownBlocks)
    } else {
        // Use XML tags for all other models including Claude
        get_grammar(GrammarType::XmlTags)
    }
}

/// Select a grammar implementation based on specified grammar type
///
/// This function returns a grammar implementation based on the provided
/// grammar type in the configuration.
///
/// # Arguments
/// * `grammar_type` - Grammar type to use
///
/// # Returns
/// A boxed Grammar implementation for the specified type
pub fn select_grammar_by_type(grammar_type: Option<grammar::formats::GrammarType>) -> Arc<dyn Grammar> {
    use grammar::formats::{get_grammar, GrammarType};
    
    // This shouldn't be None at this point as the caller should have used select_grammar_for_model
    // if they didn't have an explicit grammar type
    match grammar_type {
        Some(grammar) => get_grammar(grammar),
        None => panic!("Grammar type should be specified when calling select_grammar_by_type")
    }
}