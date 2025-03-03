//! Prompt template system
//!
//! This module handles loading, parsing, and rendering prompt templates.
//! It uses a Handlebars template system for flexible prompt composition.

mod grammar;
mod handlebars;

use std::sync::Arc;
pub use grammar::{Grammar, OldGrammar};
pub use handlebars::TemplateManager;

/// List of all available tools
pub const ALL_TOOLS: &[&str] = &[
    "shell", "read", "write", "patch", "fetch", 
    "task", "done", "agent", "wait"
];

/// List of read-only tools (excludes tools that can modify the filesystem)
pub const READONLY_TOOLS: &[&str] = &[
    "shell", "read", "fetch", "done", "agent", "wait"
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
/// * `use_minimal` - Whether to use the minimal prompt template
///
/// # Returns
/// The generated system prompt as a string
pub fn generate_system_prompt(enabled_tools: &[&str], use_minimal: bool, grammar: Arc<dyn Grammar>) -> String {
    let template_name = if use_minimal {
        "minimal"
    } else {
        "basic"
    };
    
    render_template(template_name, enabled_tools, grammar)
}

/// Select a grammar implementation based on model name
///
/// This function returns the appropriate grammar implementation
/// for the specified model. Currently, all models use OldGrammar,
/// but this can be extended in the future for model-specific behaviors.
///
/// # Arguments
/// * `model_name` - The name of the model to select grammar for
///
/// # Returns
/// A boxed Grammar implementation appropriate for the model
pub fn select_grammar_for_model(model_name: &str) -> Arc<dyn Grammar>{
    // Currently all models use the OldGrammar implementation
    // In the future, this can be extended for model-specific grammars
    let _model_lower = model_name.to_lowercase();
    
    // Example of how model-specific grammar could be implemented:
    // if _model_lower.contains("gpt-4") {
    //     return Box::new(SomeOtherGrammar {});
    // }
    
    // Default to OldGrammar for all models for now
    Arc::new(OldGrammar {})
}