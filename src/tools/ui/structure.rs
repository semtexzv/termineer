//! Structured representation of UI elements for UI tools
//!
//! This module provides a structured representation of UI elements
//! that can be serialized to XML or other formats. Used by screenshot,
//! screendump, and input tools.

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::collections::HashMap;
use std::io::Cursor;

/// A structured representation of a UI element
#[derive(Debug, Clone)]
pub struct UIElement {
    /// The role or type of the element (button, text field, etc.)
    pub element_type: String,

    /// An identifier for the element, if available
    pub identifier: Option<String>,

    /// The title or label of the element
    pub title: Option<String>,

    /// The description of the element
    pub description: Option<String>,

    /// The current value of the element, if applicable
    pub value: Option<String>,

    /// The position of the element (x, y)
    pub position: Option<(i32, i32)>,

    /// The size of the element (width, height)
    pub size: Option<(i32, i32)>,

    /// Whether the element is enabled
    pub enabled: Option<bool>,

    /// Whether the element is focused
    pub focused: Option<bool>,

    /// Whether the element is selected
    pub selected: Option<bool>,

    /// Child elements
    pub children: Vec<UIElement>,

    /// Additional attributes
    pub attributes: HashMap<String, String>,
}

impl UIElement {
    /// Create a new UI element with the given type
    pub fn new(element_type: &str) -> Self {
        UIElement {
            element_type: element_type.to_string(),
            identifier: None,
            title: None,
            description: None,
            value: None,
            position: None,
            size: None,
            enabled: None,
            focused: None,
            selected: None,
            children: Vec::new(),
            attributes: HashMap::new(),
        }
    }

    /// Convert to XML format
    #[allow(dead_code)]
    pub fn to_xml(&self) -> Result<String, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        self.write_xml_element(&mut writer, 0)?;

        let result = writer.into_inner().into_inner();

        let xml_string =
            String::from_utf8(result).map_err(|e| format!("XML encoding error: {e}"))?;

        // Log the XML string for debugging
        crate::bprintln!(dev: "🖥️ STRUCTURE: Generated XML for UIWindow:\n{xml_string}");

        Ok(xml_string)
    }

    /// Write this element to XML
    fn write_xml_element<W: std::io::Write>(
        &self,
        writer: &mut Writer<W>,
        depth: usize,
    ) -> Result<(), String> {
        // Create element
        let mut elem = BytesStart::new("UIElement");

        // Add attributes
        elem.push_attribute(("type", self.element_type.as_str()));

        if let Some(id) = &self.identifier {
            elem.push_attribute(("id", id.as_str()));
        }

        if let Some((x, y)) = self.position {
            elem.push_attribute(("x", x.to_string().as_str()));
            elem.push_attribute(("y", y.to_string().as_str()));
        }

        if let Some((width, height)) = self.size {
            elem.push_attribute(("width", width.to_string().as_str()));
            elem.push_attribute(("height", height.to_string().as_str()));
        }

        if let Some(enabled) = self.enabled {
            elem.push_attribute(("enabled", enabled.to_string().as_str()));
        }

        if let Some(focused) = self.focused {
            elem.push_attribute(("focused", focused.to_string().as_str()));
        }

        if let Some(selected) = self.selected {
            elem.push_attribute(("selected", selected.to_string().as_str()));
        }

        // Write element start
        writer
            .write_event(Event::Start(elem))
            .map_err(|e| format!("XML write error: {e}"))?;

        // Write title if present
        if let Some(title) = &self.title {
            writer
                .write_event(Event::Start(BytesStart::new("title")))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::Text(BytesText::new(title)))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::End(BytesEnd::new("title")))
                .map_err(|e| format!("XML write error: {e}"))?;
        }

        // Write description if present
        if let Some(desc) = &self.description {
            writer
                .write_event(Event::Start(BytesStart::new("description")))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::Text(BytesText::new(desc)))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::End(BytesEnd::new("description")))
                .map_err(|e| format!("XML write error: {e}"))?;
        }

        // Write value if present
        if let Some(val) = &self.value {
            writer
                .write_event(Event::Start(BytesStart::new("value")))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::Text(BytesText::new(val)))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::End(BytesEnd::new("value")))
                .map_err(|e| format!("XML write error: {e}"))?;
        }

        // Write additional attributes
        for (key, value) in &self.attributes {
            let mut attr_elem = BytesStart::new("attribute");
            attr_elem.push_attribute(("name", key.as_str()));

            writer
                .write_event(Event::Start(attr_elem))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::Text(BytesText::new(value)))
                .map_err(|e| format!("XML write error: {e}"))?;
            writer
                .write_event(Event::End(BytesEnd::new("attribute")))
                .map_err(|e| format!("XML write error: {e}"))?;
        }

        // Write children recursively
        for child in &self.children {
            child.write_xml_element(writer, depth + 1)?;
        }

        // Close element
        writer
            .write_event(Event::End(BytesEnd::new("UIElement")))
            .map_err(|e| format!("XML write error: {e}"))?;

        Ok(())
    }
}

/// A structured representation of a window
#[derive(Debug, Clone)]
pub struct UIWindow {
    /// The application name
    pub app_name: String,

    /// The window title
    pub window_title: String,

    /// The window position (x, y)
    pub position: (i32, i32),

    /// The window size (width, height)
    pub size: (i32, i32),

    /// The window's UI element tree
    pub ui_tree: Option<UIElement>,
}

impl UIWindow {
    /// Convert the window and its UI tree to XML with proper indentation
    pub fn to_xml(&self) -> Result<String, String> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));

        // XML declaration
        writer
            .write_event(Event::Decl(quick_xml::events::BytesDecl::new(
                "1.0",
                Some("UTF-8"),
                None,
            )))
            .map_err(|e| format!("XML write error: {e}"))?;

        // Root element
        writer
            .write_event(Event::Start(BytesStart::new("UITree")))
            .map_err(|e| format!("XML write error: {e}"))?;

        // Window information
        let mut window_elem = BytesStart::new("Window");
        window_elem.push_attribute(("app", self.app_name.as_str()));
        window_elem.push_attribute(("title", self.window_title.as_str()));
        window_elem.push_attribute(("x", self.position.0.to_string().as_str()));
        window_elem.push_attribute(("y", self.position.1.to_string().as_str()));
        window_elem.push_attribute(("width", self.size.0.to_string().as_str()));
        window_elem.push_attribute(("height", self.size.1.to_string().as_str()));

        writer
            .write_event(Event::Start(window_elem))
            .map_err(|e| format!("XML write error: {e}"))?;

        // UI tree if available
        if let Some(tree) = &self.ui_tree {
            tree.write_xml_element(&mut writer, 1)?;
        }

        // Close window element
        writer
            .write_event(Event::End(BytesEnd::new("Window")))
            .map_err(|e| format!("XML write error: {e}"))?;

        // Close root element
        writer
            .write_event(Event::End(BytesEnd::new("UITree")))
            .map_err(|e| format!("XML write error: {e}"))?;

        let result = writer.into_inner().into_inner();
        let raw_xml =
            String::from_utf8(result).map_err(|e| format!("XML encoding error: {e}"))?;

        // Format the XML with proper indentation
        let formatted_xml = Self::format_xml_string(&raw_xml)?;

        // Log the formatted XML string for debugging
        crate::bprintln!(dev: "🖥️ STRUCTURE: Generated XML for UIWindow:\n{formatted_xml}");

        Ok(formatted_xml)
    }

    /// Format an XML string with proper indentation
    fn format_xml_string(xml: &str) -> Result<String, String> {
        let mut formatted = String::new();
        let mut indent_level: usize = 0;
        let mut in_tag = false;
        let mut in_declaration = false;
        let mut after_open_bracket = false;

        for c in xml.chars() {
            match c {
                '<' => {
                    // Starting a new tag
                    after_open_bracket = true;
                    in_tag = true;

                    // Check if this is a closing tag
                    let is_closing =
                        xml.chars().skip(xml.len() - formatted.len()).next() == Some('/');
                    if is_closing {
                        indent_level = indent_level.saturating_sub(1);
                    }

                    // Add a new line before tags (except the first tag)
                    if !formatted.is_empty() {
                        formatted.push('\n');
                    }

                    // Add indentation
                    formatted.push_str(&"  ".repeat(indent_level));

                    // Add the character
                    formatted.push(c);
                }
                '?' => {
                    formatted.push(c);
                    if after_open_bracket {
                        in_declaration = true;
                        after_open_bracket = false;
                    }
                }
                '/' => {
                    // Self-closing tag or closing tag
                    formatted.push(c);
                    if after_open_bracket {
                        // This is a closing tag, already handled in '<' case
                        after_open_bracket = false;
                    }
                }
                '>' => {
                    // Ending a tag
                    formatted.push(c);

                    if in_declaration {
                        in_declaration = false;
                    } else if in_tag {
                        let is_self_closing = formatted.chars().rev().nth(1) == Some('/');
                        if !is_self_closing
                            && !xml
                                .chars()
                                .skip(xml.len() - formatted.len())
                                .next()
                                .unwrap_or(' ')
                                .is_whitespace()
                        {
                            // Only increase indent if this is an opening tag (not self-closing)
                            // and the next character isn't another tag
                            indent_level += 1;
                        }
                    }

                    in_tag = false;
                    after_open_bracket = false;
                }
                '\n' | '\r' => {
                    // Skip existing newlines
                }
                ' ' => {
                    // Preserve spaces in tags, compress multiple spaces in content
                    if in_tag || formatted.chars().last() != Some(' ') {
                        formatted.push(c);
                    }
                }
                _ => {
                    // Other characters
                    formatted.push(c);
                    after_open_bracket = false;
                }
            }
        }

        Ok(formatted)
    }
}
