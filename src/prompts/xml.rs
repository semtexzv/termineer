//! XML parsing for prompt templates
//!
//! This module provides XML parsing functionality for prompt templates
//! using the xmltree library.

use std::collections::HashMap;
use thiserror::Error;
use xmltree::{Element, XMLNode};

use super::template::{Section, Template, TemplateMeta, TemplateError};
use super::variables::Variables;

/// Errors that can occur during XML parsing
#[derive(Error, Debug)]
pub enum XmlError {
    #[error("XML parse error: {0}")]
    Parse(#[from] xmltree::ParseError),
    
    #[error("Missing required element: {0}")]
    MissingElement(String),
    
    #[error("Invalid attribute value: {0}")]
    InvalidAttribute(String),
    
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<XmlError> for TemplateError {
    fn from(err: XmlError) -> Self {
        match err {
            XmlError::Parse(e) => TemplateError::XmlParse(e.to_string()),
            XmlError::MissingElement(e) => TemplateError::XmlParse(format!("Missing element: {}", e)),
            XmlError::InvalidAttribute(e) => TemplateError::XmlParse(format!("Invalid attribute: {}", e)),
            XmlError::Io(e) => TemplateError::Io(e),
        }
    }
}

/// Parse an XML template string into a Template struct
pub fn parse_template(xml: &str) -> Result<Template, TemplateError> {
    // Parse the XML document
    let root = Element::parse(xml.as_bytes())
        .map_err(|e| TemplateError::XmlParse(format!("Failed to parse XML: {}", e)))?;
    
    // Verify we have a template element
    if root.name != "template" {
        return Err(TemplateError::XmlParse("Root element is not 'template'".to_string()));
    }
    
    // Extract template ID
    let template_id = root.attributes.get("id")
        .ok_or_else(|| TemplateError::XmlParse("Missing template 'id' attribute".to_string()))?
        .clone();
    
    // Extract metadata
    let mut meta = TemplateMeta::default();
    if let Some(meta_elem) = root.get_child("meta") {
        // Extract name, version, description
        if let Some(name_elem) = meta_elem.get_child("name") {
            meta.name = get_element_text(name_elem).unwrap_or_default();
        }
        
        if let Some(version_elem) = meta_elem.get_child("version") {
            meta.version = get_element_text(version_elem).unwrap_or_default();
        }
        
        if let Some(desc_elem) = meta_elem.get_child("description") {
            meta.description = get_element_text(desc_elem).unwrap_or_default();
        }
    }
    
    // Extract sections
    let mut sections = HashMap::new();
    for child in &root.children {
        if let XMLNode::Element(element) = child {
            if element.name == "section" {
                // Get section ID
                if let Some(id) = element.attributes.get("id") {
                    // Get section content
                    let content = get_element_text(element).unwrap_or_default();
                    
                    // Add section
                    sections.insert(
                        id.clone(),
                        Section {
                            id: id.clone(),
                            content: content.trim().to_string(),
                        },
                    );
                }
            }
        }
    }
    
    // Extract variables
    let mut variables = Variables::new();
    if let Some(vars_elem) = root.get_child("variables") {
        for child in &vars_elem.children {
            if let XMLNode::Element(element) = child {
                if element.name == "variable" {
                    // Get variable ID
                    if let Some(id) = element.attributes.get("id") {
                        // Get value or default
                        if let Some(value) = element.attributes.get("value") {
                            variables.set(id, value);
                        } else if let Some(default) = element.attributes.get("default") {
                            variables.set(id, default);
                        }
                    }
                }
            }
        }
    }
    
    // Extract prompt template
    let prompt_template = root.get_child("_prompt")
        .map(get_element_text)
        .flatten();
    
    // Load global variables and merge with template variables
    if let Ok(global_vars) = Variables::load_globals() {
        for (name, value) in global_vars.iter() {
            if variables.get(name).is_none() {
                variables.set(name, value);
            }
        }
    }
    
    // Extract tool elements
    let mut tool_elements = Vec::new();
    let extracted_tools = extract_tool_elements(&root);
    for (name, attributes, content) in extracted_tools {
        tool_elements.push(super::template::ToolElement {
            name,
            attributes,
            content,
        });
    }
    
    Ok(Template {
        id: template_id,
        meta,
        sections,
        variables,
        prompt_template,
        tool_elements,
    })
}

/// Helper function to extract text content from an XML element
fn get_element_text(element: &Element) -> Option<String> {
    let mut text = String::new();
    
    for child in &element.children {
        match child {
            XMLNode::Text(content) => text.push_str(content),
            XMLNode::Element(elem) => {
                // Special handling for g-tool elements
                if elem.name == "g-tool" {
                    // We'll process these during rendering with Grammar trait
                    // Just include a placeholder for now
                    let tool_name = elem.attributes.get("name").map_or("", |s| s);
                    let tool_placeholder = format!("{{__TOOL_{}__}}", tool_name);
                    text.push_str(&tool_placeholder);
                } else {
                    // Recursively get text from child elements
                    if let Some(child_text) = get_element_text(elem) {
                        text.push_str(&child_text);
                    }
                }
            }
            _ => {}
        }
    }
    
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Extract tool elements from an XML element
fn extract_tool_elements(element: &Element) -> Vec<(String, HashMap<String, String>, String)> {
    let mut tools = Vec::new();
    
    // Process this element if it's a g-tool
    if element.name == "g-tool" {
        let name = element.attributes.get("name").map_or("", |s| s.as_str()).to_string();
        let content = get_element_text(element).unwrap_or_default();
        tools.push((name, element.attributes.clone(), content));
    }
    
    // Recursively process child elements
    for child in &element.children {
        if let XMLNode::Element(elem) = child {
            let mut child_tools = extract_tool_elements(elem);
            tools.append(&mut child_tools);
        }
    }
    
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_basic_template() {
        let xml = r#"<template id="test">
            <meta>
                <name>Test Template</name>
                <version>1.0</version>
                <description>A simple test template</description>
            </meta>
            <section id="introduction">
                Hello, world!
            </section>
            <variables>
                <variable id="Name" value="User" />
                <variable id="Greeting" default="Welcome" />
            </variables>
            <_prompt>
                {introduction}
                {Greeting}, {Name}!
            </_prompt>
        </template>"#;
        
        let template = parse_template(xml).unwrap();
        
        assert_eq!(template.id, "test");
        assert_eq!(template.meta.name, "Test Template");
        assert_eq!(template.meta.version, "1.0");
        assert!(template.sections.contains_key("introduction"));
        assert_eq!(template.sections["introduction"].content, "Hello, world!");
        
        // Check variables
        assert_eq!(template.variables.get("Name").unwrap(), "User");
        assert_eq!(template.variables.get("Greeting").unwrap(), "Welcome");
        
        // Check prompt template
        assert!(template.prompt_template.is_some());
        assert!(template.prompt_template.unwrap().contains("{introduction}"));
    }
}