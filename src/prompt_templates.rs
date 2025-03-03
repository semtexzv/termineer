//! XML-based prompt template system
//!
//! This module provides functionality for loading and rendering XML-based
//! prompt templates with section references and variable substitution.

use quick_xml::events::{Event, BytesStart};
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::{BufRead, BufReader};

/// Error type for template operations
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("XML parsing error: {0}")]
    XmlParse(#[from] quick_xml::Error),
    
    #[error("Template not found: {0}")]
    NotFound(String),
    
    #[error("Section not found: {0}")]
    SectionNotFound(String),
    
    #[error("Prompt section missing")]
    PromptMissing,
    
    #[error("Other error: {0}")]
    Other(String),
}

/// Struct representing an XML prompt template
#[derive(Debug)]
pub struct XmlTemplate {
    /// Template ID
    pub id: String,
    
    /// Metadata about the template
    pub metadata: HashMap<String, String>,
    
    /// Sections defined in the template
    pub sections: HashMap<String, String>,
    
    /// Variables with default values
    pub variables: HashMap<String, String>,
    
    /// The final prompt format using section references
    pub prompt_format: String,
}

/// Engine for loading and rendering XML templates
pub struct TemplateEngine {
    /// Base directory for template files
    template_dir: PathBuf,
    
    /// Cached templates
    templates: HashMap<String, XmlTemplate>,
}

impl TemplateEngine {
    /// Create a new template engine with the specified template directory
    pub fn new(template_dir: &str) -> Self {
        Self {
            template_dir: PathBuf::from(template_dir),
            templates: HashMap::new(),
        }
    }
    
    /// Load a template by name
    pub fn load_template(&mut self, name: &str) -> Result<&XmlTemplate, TemplateError> {
        if !self.templates.contains_key(name) {
            let path = self.template_dir.join(format!("{}.xml", name));
            let template = self.parse_template_file(&path)?;
            self.templates.insert(name.to_string(), template);
        }
        
        Ok(&self.templates[name])
    }
    
    /// Render a template with the specified variables
    pub fn render(&mut self, name: &str, vars: Option<HashMap<String, String>>) 
        -> Result<String, TemplateError> 
    {
        // Load template if not already loaded
        let template = self.load_template(name)?;
        
        // Start with template's default variables
        let mut effective_vars = template.variables.clone();
        
        // Override with provided variables
        if let Some(provided_vars) = vars {
            for (key, value) in provided_vars {
                effective_vars.insert(key, value);
            }
        }
        
        // Start with the prompt format
        let mut result = template.prompt_format.clone();
        
        // Replace section references ${section_id}
        for (id, content) in &template.sections {
            let placeholder = format!("${{{}}}", id);
            result = result.replace(&placeholder, content);
        }
        
        // Replace variable references ${variable_id}
        for (key, value) in &effective_vars {
            let placeholder = format!("${{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        // Add tool syntax replacement - convert placeholders to actual syntax
        result = result.replace("[TOOL_START]", "<tool>");
        result = result.replace("[TOOL_END]", "