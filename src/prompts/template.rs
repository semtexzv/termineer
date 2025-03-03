//! Template definition and rendering
//!
//! This module defines the Template structure and handles
//! loading, parsing, and rendering prompt templates.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::variables::Variables;
use super::xml;

/// Errors that can occur during template operations
#[derive(Error, Debug)]
pub enum TemplateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("XML parsing error: {0}")]
    XmlParse(String),
    
    #[error("Template '{0}' not found")]
    TemplateNotFound(String),
    
    #[error("Section '{0}' not found in template")]
    SectionNotFound(String),
    
    #[error("Invalid template format: {0}")]
    InvalidFormat(String),
}

/// A template section containing content
#[derive(Debug, Clone)]
pub struct Section {
    /// Unique identifier for the section
    pub id: String,
    /// The section content
    pub content: String,
}

/// Template metadata
#[derive(Debug, Clone, Default)]
pub struct TemplateMeta {
    /// Template name
    pub name: String,
    /// Template version
    pub version: String,
    /// Template description
    pub description: String,
}

/// Grammar element type
#[derive(Debug, Clone, PartialEq)]
pub enum GrammarElementType {
    /// Regular tool invocation
    Tool,
    /// Tool result message
    Done,
    /// Tool error message
    Error,
}

/// Grammar element extracted from the template
#[derive(Debug, Clone)]
pub struct GrammarElement {
    /// Element type
    pub element_type: GrammarElementType,
    /// Element name (tool name)
    pub name: String,
    /// Element index (for done/error elements)
    pub index: Option<usize>,
    /// Element content
    pub content: String,
}

/// A prompt template parsed from XML
#[derive(Debug, Clone)]
pub struct Template {
    /// Template ID
    pub id: String,
    /// Template metadata
    pub meta: TemplateMeta,
    /// Template sections
    pub sections: HashMap<String, Section>,
    /// Default template variables
    pub variables: Variables,
    /// The composed prompt template (if specified)
    pub prompt_template: Option<String>,
    /// Grammar elements extracted from the template
    pub grammar_elements: Vec<GrammarElement>,
}

impl Template {
    /// Load a template from a file path
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, TemplateError> {
        let content = fs::read_to_string(path)?;
        Self::from_xml(&content)
    }
    
    /// Parse a template from XML content
    pub fn from_xml(xml_content: &str) -> Result<Self, TemplateError> {
        // Use the new XML parser module
        xml::parse_template(xml_content)
    }
    
    /// Get a section by ID
    pub fn get_section(&self, id: &str) -> Option<&Section> {
        self.sections.get(id)
    }
    
    /// Render the template with variable substitution
    pub fn render(&self, variables: &Variables) -> Result<String, TemplateError> {
        // If we have a prompt template, use it
        if let Some(prompt_template) = &self.prompt_template {
            let mut result = prompt_template.clone();
            
            // Apply substitutions until content stabilizes
            let mut prev_result;
            let mut iteration_count = 0;
            let max_iterations = 10; // Prevent infinite loops
            
            loop {
                prev_result = result.clone();
                
                // First, process section references
                for (id, section) in &self.sections {
                    let pattern = format!("{{{}}}", id);
                    // Also process section content to handle nested references
                    let processed_content = self.process_content(&section.content, variables)?;
                    result = result.replace(&pattern, &processed_content);
                }
                
                // Then substitute variables
                result = variables.substitute(&result);
                
                // Check if content has stabilized (no more substitutions happened)
                if result == prev_result || iteration_count >= max_iterations {
                    break;
                }
                
                iteration_count += 1;
            }
            
            // Check if we stopped due to max iterations (potential circular references)
            if iteration_count >= max_iterations {
                return Err(TemplateError::InvalidFormat(
                    "Potential circular reference detected in template".to_string()
                ));
            }
            
            Ok(result)
        } else {
            // Otherwise, just concatenate all processed sections
            let processed_sections: Result<Vec<String>, TemplateError> = self.sections.values()
                .map(|section| self.process_content(&section.content, variables))
                .collect();
                
            let combined = processed_sections?
                .join("\n\n");
                
            Ok(combined)
        }
    }
    
    /// Render the template with variable substitution and grammar processing
    pub fn render_with_grammar(&self, variables: &Variables, grammar: &dyn crate::prompts::Grammar) -> Result<String, TemplateError> {
        // First, do the standard rendering to substitute variables and sections
        let rendered = self.render(variables)?;
        
        // Then process any grammar element placeholders
        let mut result = rendered;
        
        // Process each grammar element
        for element in &self.grammar_elements {
            match element.element_type {
                GrammarElementType::Tool => {
                    let placeholder = format!("{{__TOOL_{}__}}", element.name);
                    
                    // Format the tool using the grammar - only passing name and content
                    // The tool itself will parse arguments from the content
                    let formatted_element = grammar.format_tool_call(&element.name, &element.content);
                    
                    // Replace the placeholder with the formatted tool
                    result = result.replace(&placeholder, &formatted_element);
                },
                GrammarElementType::Done => {
                    // Use the index if available, otherwise default to 0
                    let index = element.index.unwrap_or(0);
                    let placeholder = format!("{{__DONE_{}}}", index);
                    
                    // Format the result using the grammar
                    let formatted_element = grammar.format_tool_result(index, &element.content);
                    
                    // Replace the placeholder with the formatted result
                    result = result.replace(&placeholder, &formatted_element);
                },
                GrammarElementType::Error => {
                    // Use the index if available, otherwise default to 0
                    let index = element.index.unwrap_or(0);
                    let placeholder = format!("{{__ERROR_{}}}", index);
                    
                    // Format the error using the grammar
                    let formatted_element = grammar.format_tool_error(index, &element.content);
                    
                    // Replace the placeholder with the formatted error
                    result = result.replace(&placeholder, &formatted_element);
                }
            }
        }
        
        Ok(result)
    }
    
    /// Process content by recursively applying section and variable substitutions
    fn process_content(&self, content: &str, variables: &Variables) -> Result<String, TemplateError> {
        let mut result = content.to_string();
        let mut prev_result;
        let mut iteration_count = 0;
        let max_iterations = 5; // Limit recursion depth
        
        loop {
            prev_result = result.clone();
            
            // Replace section references
            for (id, section) in &self.sections {
                let pattern = format!("{{{}}}", id);
                result = result.replace(&pattern, &section.content);
            }
            
            // Replace variables
            result = variables.substitute(&result);
            
            // Check if content has stabilized
            if result == prev_result || iteration_count >= max_iterations {
                break;
            }
            
            iteration_count += 1;
        }
        
        // Check for potential circular references
        if iteration_count >= max_iterations {
            return Err(TemplateError::InvalidFormat(
                "Potential circular reference detected in content".to_string()
            ));
        }
        
        Ok(result)
    }
    
    /// Find a template in the templates directory by ID
    pub fn find_by_id(id: &str) -> Result<Self, TemplateError> {
        let templates_dir = PathBuf::from("prompts");
        
        // Try to find a template file with the matching ID
        for entry in fs::read_dir(templates_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| ext == "xml") {
                // Try to parse the file
                if let Ok(template) = Self::from_file(&path) {
                    // Check if this is the template we're looking for
                    if template.id == id {
                        return Ok(template);
                    }
                }
            }
        }
        
        Err(TemplateError::TemplateNotFound(id.to_string()))
    }
    
    /// Find and load a template file by name
    pub fn from_name(name: &str) -> Result<Self, TemplateError> {
        let templates_dir = PathBuf::from("prompts");
        let filename = format!("{}.xml", name);
        let path = templates_dir.join(filename);
        
        if path.exists() {
            Self::from_file(path)
        } else {
            Err(TemplateError::TemplateNotFound(name.to_string()))
        }
    }
    
    /// Merge additional variables into the template's variables
    pub fn with_variables(&mut self, additional: &Variables) -> &mut Self {
        for (name, value) in additional.iter() {
            self.variables.set(name, value);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_render_template() {
        // Create a simple template for testing
        let mut template = Template {
            id: "test".to_string(),
            meta: TemplateMeta {
                name: "Test Template".to_string(),
                version: "1.0".to_string(),
                description: "A test template".to_string(),
            },
            sections: HashMap::new(),
            variables: Variables::new(),
            prompt_template: Some("{introduction}\n\n{body}".to_string()),
            grammar_elements: Vec::new(),
        };
        
        // Add sections
        template.sections.insert(
            "introduction".to_string(), 
            Section {
                id: "introduction".to_string(),
                content: "Hello, {Name}!".to_string(),
            }
        );
        
        template.sections.insert(
            "body".to_string(), 
            Section {
                id: "body".to_string(),
                content: "This is a {Type} test.".to_string(),
            }
        );
        
        // Create variables
        let mut vars = Variables::new();
        vars.set("Name", "World");
        vars.set("Type", "simple");
        
        // Render the template
        let result = template.render(&vars).unwrap();
        assert_eq!(result, "Hello, World!\n\nThis is a simple test.");
    }
}