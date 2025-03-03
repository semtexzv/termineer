//! Prompt template system
//!
//! This module handles loading, parsing, and rendering prompt templates.
//! It uses an XML-based template system that allows for flexible prompt composition.

mod template;
mod variables;
mod xml;
mod grammar;
pub mod old;

// Re-export key components
pub use template::{Template, TemplateError};
pub use variables::Variables;
pub use grammar::{Grammar, OldGrammar};

/// Re-export ToolDocOptions from the old system for compatibility
pub use self::old::ToolDocOptions;

/// Generate a system prompt with appropriate tool documentation
/// 
/// This function now uses the template-based system with Grammar integration
pub fn generate_system_prompt(options: &ToolDocOptions, grammar: &dyn Grammar) -> String {
    // Try to use the template-based system first
    match generate_basic_from_template(options, grammar) {
        Ok(prompt) => prompt,
        Err(_) => {
            // Fall back to the old system
            self::old::generate_system_prompt(options, grammar)
        }
    }
}

/// Generate a minimal system prompt with appropriate tool documentation
/// 
/// This function now uses the template-based system with Grammar integration
pub fn generate_minimal_system_prompt(options: &ToolDocOptions, grammar: &dyn Grammar) -> String {
    // Try to use the template-based system first
    match generate_minimal_from_template(options, grammar) {
        Ok(prompt) => prompt,
        Err(_) => {
            // Fall back to the old system
            self::old::generate_minimal_system_prompt(options, grammar)
        }
    }
}

/// Format the subagent prompt with creator information
/// 
/// This function now uses the template-based system with Grammar integration
pub fn format_subagent_prompt(creator_name: &str, creator_id: &str, grammar: &dyn Grammar) -> String {
    // Try to use the template-based system first
    let options = ToolDocOptions::default();
    match format_subagent_from_template(creator_name, creator_id, &options, grammar) {
        Ok(prompt) => prompt,
        Err(_) => {
            // Fall back to the old system
            self::old::format_subagent_prompt(creator_name, creator_id)
        }
    }
}

// New template-based functions

/// Load a prompt template by name
///
/// This looks for an XML file in the prompts directory with the given name
/// (automatically adding the .xml extension)
pub fn load_template(name: &str) -> Result<Template, TemplateError> {
    Template::from_name(name)
}

/// Load a prompt template by ID
///
/// This searches all XML files in the prompts directory for a template
/// with the matching ID attribute
pub fn find_template_by_id(id: &str) -> Result<Template, TemplateError> {
    Template::find_by_id(id)
}

/// Generate a prompt using the basic template
///
/// This loads the 'basic.xml' template and renders it with the provided tools options
/// and grammar implementation.
pub fn generate_from_template(template_name: &str, options: &ToolDocOptions, grammar: &dyn Grammar) -> Result<String, TemplateError> {
    // Load the specified template
    let mut template = load_template(template_name)?;
    
    // Create variables based on the tool options
    let mut vars = Variables::new();
    
    // Add tool-related variables
    vars.set("IncludeShell", if options.include_shell { "true" } else { "false" });
    vars.set("IncludeRead", if options.include_read { "true" } else { "false" });
    vars.set("IncludeWrite", if options.include_write { "true" } else { "false" });
    vars.set("IncludePatch", if options.include_patch { "true" } else { "false" });
    vars.set("IncludeFetch", if options.include_fetch { "true" } else { "false" });
    vars.set("IncludeTask", if options.include_task { "true" } else { "false" });
    vars.set("IncludeAgent", if options.include_agent { "true" } else { "false" });
    vars.set("IncludeWait", if options.include_wait { "true" } else { "false" });
    vars.set("IncludeDone", if options.include_done { "true" } else { "false" });
    
    // Merge with template variables
    template.with_variables(&vars);
    
    // Render with grammar support
    template.render_with_grammar(&template.variables, grammar)
}

/// Generate a minimal prompt using the minimal template
///
/// This loads the 'minimal.xml' template and renders it with the provided tools options
/// and grammar implementation.
pub fn generate_minimal_from_template(options: &ToolDocOptions, grammar: &dyn Grammar) -> Result<String, TemplateError> {
    generate_from_template("minimal", options, grammar)
}

/// Generate a standard prompt using the basic template
///
/// This loads the 'basic.xml' template and renders it with the provided tools options
/// and grammar implementation.
pub fn generate_basic_from_template(options: &ToolDocOptions, grammar: &dyn Grammar) -> Result<String, TemplateError> {
    generate_from_template("basic", options, grammar)
}

/// Format a subagent prompt using templates
///
/// This loads the 'basic.xml' template, adds creator information as variables,
/// and renders it with the provided tools options and grammar implementation.
pub fn format_subagent_from_template(
    creator_name: &str, 
    creator_id: &str,
    options: &ToolDocOptions,
    grammar: &dyn Grammar
) -> Result<String, TemplateError> {
    // Load the basic template
    let mut template = load_template("basic")?;
    
    // Create variables based on the tool options and creator info
    let mut vars = Variables::new();
    
    // Add creator variables
    vars.set("CreatorName", creator_name);
    vars.set("CreatorId", creator_id);
    vars.set("IsSubagent", "true");
    
    // Add tool-related variables
    vars.set("IncludeShell", if options.include_shell { "true" } else { "false" });
    vars.set("IncludeRead", if options.include_read { "true" } else { "false" });
    vars.set("IncludeWrite", if options.include_write { "true" } else { "false" });
    vars.set("IncludePatch", if options.include_patch { "true" } else { "false" });
    vars.set("IncludeFetch", if options.include_fetch { "true" } else { "false" });
    vars.set("IncludeTask", if options.include_task { "true" } else { "false" });
    vars.set("IncludeAgent", if options.include_agent { "true" } else { "false" });
    vars.set("IncludeWait", if options.include_wait { "true" } else { "false" });
    vars.set("IncludeDone", if options.include_done { "true" } else { "false" });
    
    // Merge with template variables and render with grammar
    template.with_variables(&vars);
    template.render_with_grammar(&template.variables, grammar)
}