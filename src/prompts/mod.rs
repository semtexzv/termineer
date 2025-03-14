#![allow(dead_code)]
//! Prompt template system
//!
//! This module handles loading, parsing, and rendering prompt templates.
//! It uses a Handlebars template system for flexible prompt composition.

pub mod grammar;
pub mod handlebars;

// Protected prompts module for encrypted templates
pub mod protected;

use anyhow::bail;
pub use grammar::{Grammar, XmlGrammar};
pub use handlebars::TemplateManager;
use std::sync::Arc;

// Include the generated list of available kinds from build script
include!(concat!(env!("OUT_DIR"), "/encrypted_prompts.rs"));

/// List of all available tools
pub const ALL_TOOLS: &[&str] = &[
    "shell", "read", "write", "patch", "fetch", "search", "mcp", "task", "done", "wait",
];

/// List of tools available to Plus/Pro users only
pub const PLUS_TOOLS: &[&str] = &["agent"];

/// List of read-only tools (excludes tools that can modify the filesystem)
pub const READONLY_TOOLS: &[&str] = &[
    "shell", "read", "fetch", "search", "mcp", "done", "wait",
];

/// List of read-only tools for Plus/Pro users
pub const READONLY_PLUS_TOOLS: &[&str] = &["agent"];

/// Check if a kind name is available in the compiled templates
pub fn is_valid_kind(kind_name: &str) -> bool {
    AVAILABLE_KINDS_ARRAY
        .iter()
        .position(|it| it == &kind_name)
        .is_some()
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
pub fn render_template(
    template_name: &str,
    enabled_tools: &[&str],
    grammar: Arc<dyn Grammar>,
) -> anyhow::Result<String> {
    // Create a template manager
    let mut template_manager = TemplateManager::new(grammar);

    // Get the list of configured MCP servers
    let mcp_servers = match crate::mcp::config::get_server_list() {
        Ok(servers) => servers,
        Err(_) => Vec::new(), // Empty list if there's an error
    };
    
    // Load all templates to ensure partials are available
    match template_manager.load_all_templates() {
        Ok(_) => {
            // Render the template with specified tools enabled and MCP servers
            match template_manager.render_with_context(template_name, enabled_tools, &mcp_servers) {
                Ok(rendered) => Ok(rendered),
                Err(_) => {
                    bail!("Error generating system prompt: {}", template_name);
                }
            }
        }
        Err(e) => bail!("Error loading templates: {}", e),
    }
}

use crate::config::{get_app_mode, AppMode};

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
    grammar: Arc<dyn Grammar>,
) -> Result<String, anyhow::Error> {
    // Determine which template to use
    let requested_kind = if let Some(name) = kind_name {
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

    // Check if the requested kind is allowed for the current app mode
    let kind = check_kind_access(requested_kind)?;

    let kind = format!("kind/{}", kind);

    // Determine if we're using read-only tools
    let using_readonly = {
        let readonly_set: std::collections::HashSet<&str> = READONLY_TOOLS.iter().copied().collect();
        let enabled_set: std::collections::HashSet<&str> = enabled_tools.iter().copied().collect();
        readonly_set == enabled_set
    };
    
    // Get the current app mode
    let app_mode = crate::config::get_app_mode();
    
    // Determine if the user has Plus or Pro subscription
    let has_plus = matches!(
        app_mode,
        crate::config::AppMode::Plus | crate::config::AppMode::Pro
    );
    
    // Create a set of premium tools for filtering
    let premium_tools: std::collections::HashSet<&str> = PLUS_TOOLS.iter().copied().collect();
    
    // Combine standard tools with appropriate Plus tools based on subscription and read-only mode
    let mut combined_tools = Vec::with_capacity(
        enabled_tools.len() + 
        if has_plus { 
            if using_readonly { READONLY_PLUS_TOOLS.len() } else { PLUS_TOOLS.len() } 
        } else { 0 }
    );
    
    // Add standard tools, filtering out premium tools for free users
    for tool in enabled_tools {
        // For Free users, skip premium tools even if they're passed in enabled_tools
        if !has_plus && premium_tools.contains(tool) {
            continue;
        }
        combined_tools.push(*tool);
    }
    
    // If user has Plus/Pro, add appropriate Plus-only tools
    if has_plus {
        if using_readonly {
            for tool in READONLY_PLUS_TOOLS {
                // Avoid adding duplicates
                if !combined_tools.contains(tool) {
                    combined_tools.push(*tool);
                }
            }
        } else {
            for tool in PLUS_TOOLS {
                // Avoid adding duplicates
                if !combined_tools.contains(tool) {
                    combined_tools.push(*tool);
                }
            }
        }
    }

    // Render the template with the complete tool list
    match render_template(&kind, &combined_tools, grammar) {
        Ok(prompt) => Ok(prompt),
        Err(e) => {
            bail!("Failed to render template '{}': {}", kind, e);
        }
    }
}

/// Check if the requested kind is allowed for the current app mode
/// Returns the appropriate kind to use (either the requested one or a fallback)
fn check_kind_access(requested_kind: &str) -> Result<String, anyhow::Error> {
    // Get the current app mode from the global state
    let app_mode = get_app_mode();

    // Check for plus/ and pro/ prefixes in the kind name
    if requested_kind.starts_with("plus/") && matches!(app_mode, AppMode::Free) {
        bail!("The '{}' agent kind requires a Plus or Pro subscription.", requested_kind);
    }

    if requested_kind.starts_with("pro/") {
        match app_mode {
            AppMode::Free | AppMode::Plus => {
                bail!("The '{}' agent kind requires a Pro subscription.", requested_kind);
            }
            AppMode::Pro => {
                // Pro users can access pro templates
            }
        }
    }

    // Kind is allowed for the current app mode
    Ok(requested_kind.to_string())
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
    use grammar::formats::{get_grammar_by_type, GrammarType};

    // Choose grammar based on model name
    let model_lower = model_name.to_lowercase();

    if model_lower.contains("gemini") {
        // Use markdown grammar for Gemini models
        get_grammar_by_type(GrammarType::MarkdownBlocks)
    } else {
        // Use XML tags for all other models including Claude
        get_grammar_by_type(GrammarType::XmlTags)
    }
}
