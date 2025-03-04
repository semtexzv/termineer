//! Prompt template system
//!
//! This module handles loading, parsing, and rendering prompt templates.
//! It uses a Handlebars template system for flexible prompt composition.

pub mod grammar;
mod handlebars;

use std::sync::Arc;
pub use grammar::{Grammar, XmlGrammar};
pub use handlebars::TemplateManager;

/// List of all available tools
pub const ALL_TOOLS: &[&str] = &[
    "shell", "read", "write", "patch", "fetch", "search",
    "task", "done", "agent", "wait"
];

/// List of read-only tools (excludes tools that can modify the filesystem)
pub const READONLY_TOOLS: &[&str] = &[
    "shell", "read", "fetch", "search", "done", "agent", "wait"
];

/// Get the default list of enabled tools
pub fn default_tools() -> Vec<String> {
    ALL_TOOLS.iter().map(|&s| s.to_string()).collect()
}

/// Get the read-only list of enabled tools
pub fn readonly_tools() -> Vec<String> {
    READONLY_TOOLS.iter().map(|&s| s.to_string()).collect()
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
pub fn render_template(template_name: &str, enabled_tools: &[&str], grammar: Arc<dyn Grammar>) -> String {
    
    // Create a template manager
    let mut template_manager = TemplateManager::new(grammar);
    
    // Load all templates to ensure partials are available
    match template_manager.load_all_templates() {
        Ok(_) => {
            // Render the template with specified tools enabled
            match template_manager.render_with_tool_enablement(template_name, enabled_tools) {
                Ok(rendered) => rendered,
                Err(e) => format!("Error rendering template: {}", e),
            }
        },
        Err(e) => format!("Error loading templates: {}", e),
    }
}

/// Generate a system prompt with appropriate tool documentation
///
/// # Arguments
/// * `enabled_tools` - The list of tool names to enable
/// * `use_minimal` - Whether to use the minimal prompt template (legacy behavior)
/// * `template_name` - Optional specific template to use (overrides use_minimal)
///
/// # Returns
/// The generated system prompt as a string
pub fn generate_system_prompt(enabled_tools: &[&str], use_minimal: bool, template_name: Option<&str>, grammar: Arc<dyn Grammar>) -> String {
    // Determine which template to use
    let template = if let Some(name) = template_name {
        // If a specific template is provided, use it
        name
    } else if use_minimal {
        // Legacy behavior: if minimal is requested, use minimal template
        "minimal"
    } else {
        // Default to basic template
        "basic"
    };
    
    render_template(template, enabled_tools, grammar)
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

/// Select a grammar implementation based on configuration
///
/// This function returns a grammar implementation based on the provided
/// grammar type in the configuration.
///
/// # Arguments
/// * `grammar_type` - The type of grammar to use from configuration
///
/// # Returns
/// A boxed Grammar implementation for the specified type
pub fn select_grammar_by_type(grammar_type: grammar::formats::GrammarType) -> Arc<dyn Grammar> {
    use grammar::formats::get_grammar;
    get_grammar(grammar_type)
}