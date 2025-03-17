//! XML conversion functionality for macOS screendump
//!
//! This module provides functions to convert macOS accessibility elements
//! to structured UI element trees that can be serialized to XML.

use crate::tools::ui::structure::{UIElement, UIWindow};
use accessibility_ng::{AXUIElement, AXUIElementAttributes};
use core_graphics_types::geometry::CGPoint;
use core_graphics_types::geometry::CGSize;

/// Create a structured UIWindow from macOS elements
pub fn create_ui_window_from_macos_window(
    app_name: String,
    window_title: String,
    position: (i32, i32),
    size: (i32, i32),
    window_element: &AXUIElement,
) -> UIWindow {
    // Create the window structure with cloned values
    let mut ui_window = UIWindow {
        app_name: app_name.clone(),
        window_title: window_title.clone(),
        position,
        size,
        ui_tree: None,
    };

    // Get children elements using the children() method
    crate::bprintln!(dev: "🖥️ SCREENDUMP: Fetching UI elements for window");

    if let Ok(children) = window_element.children() {
        if !children.is_empty() {
            crate::bprintln!(dev: "🖥️ SCREENDUMP: Found {} UI elements", children.len());

            // Create a root UI element to hold all window elements
            let mut root_element = UIElement::new("Window");

            // Add window properties
            root_element
                .attributes
                .insert("app_name".to_string(), app_name.clone());
            root_element
                .attributes
                .insert("title".to_string(), window_title.clone());

            // Process child elements
            for child in &children {
                let child_element = convert_element_to_structured(&child, 5);
                root_element.children.push(child_element);
            }

            // Log the number of child elements processed
            crate::bprintln!(dev: "🖥️ XML_HELPER: Processed {} child elements for window '{}'", 
                             root_element.children.len(), window_title);

            // Set the UI tree
            ui_window.ui_tree = Some(root_element);
        } else {
            // Create an empty root element
            let root_element = UIElement::new("Window");
            ui_window.ui_tree = Some(root_element);
        }
    } else {
        // Create an empty root element
        let root_element = UIElement::new("Window");
        ui_window.ui_tree = Some(root_element);
    }

    ui_window
}

/// Convert an AXUIElement to a structured UIElement
pub fn convert_element_to_structured(element: &AXUIElement, max_depth: usize) -> UIElement {
    // Get role - this determines the element type
    let element_type = element
        .role()
        .map(|role| role.to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    // Log the element type we're processing
    crate::bprintln!(dev: "🖥️ XML_HELPER: Converting element of type '{}' (depth: {})", 
                     element_type, max_depth);

    // Create a new UI element with the determined type
    let mut ui_element = UIElement::new(&element_type);

    // Get description
    if let Ok(desc) = element.description() {
        let desc_str = desc.to_string();
        if !desc_str.is_empty() {
            ui_element.description = Some(desc_str);
        }
    }

    // Get title (might be in different attributes depending on the element type)
    if let Ok(title) = element.title() {
        let title_str = title.to_string();
        if !title_str.is_empty() {
            ui_element.title = Some(title_str);
        }
    }

    // Get value
    if let Ok(value) = element.value() {
        // Try to format the value as a string
        ui_element.value = Some(format!("{:?}", value));
    }

    // Get position - directly extract it without intermediate methods
    if let Ok(position) = element.position() {
        // Try to get the CGPoint directly from AXValue
        if let Ok(point) = position.get_value::<CGPoint>() {
            ui_element.position = Some((point.x as i32, point.y as i32));
        }
    }

    // Get size - directly extract it without intermediate methods
    if let Ok(size) = element.size() {
        // Try to get the CGSize directly from AXValue
        if let Ok(sz) = size.get_value::<CGSize>() {
            ui_element.size = Some((sz.width as i32, sz.height as i32));
        }
    }

    // Get enabled state - for now, just check that the property exists
    if let Ok(_) = element.enabled() {
        // For simplicity, we'll assume the element is enabled if the property exists
        // In a proper implementation, we would check the boolean value
        ui_element.enabled = Some(true);
    } else {
        ui_element.enabled = Some(false);
    }

    // Get focused state - for now, just check that the property exists
    if let Ok(_) = element.focused() {
        // For simplicity, we'll assume the element is focused if the property exists
        // In a proper implementation, we would check the boolean value
        ui_element.focused = Some(true);
    } else {
        ui_element.focused = Some(false);
    }

    // Get identifier if available
    if let Ok(identifier) = element.identifier() {
        let id_str = identifier.to_string();
        if !id_str.is_empty() {
            ui_element.identifier = Some(id_str);
        }
    }

    // Only process children if we haven't reached max depth
    if max_depth > 0 {
        if let Ok(children) = element.children() {
            // Log the number of children found
            if !children.is_empty() {
                crate::bprintln!(dev: "🖥️ XML_HELPER: Found {} child elements for {} element (processing depth: {})", 
                                 children.len(), element_type, max_depth);
            }

            // Process each child element recursively
            for (i, child) in children.iter().enumerate() {
                // Log progress for large element trees
                if children.len() > 10 && i % 10 == 0 {
                    crate::bprintln!(dev: "🖥️ XML_HELPER: Processing child {}/{} for {} element", 
                                     i+1, children.len(), element_type);
                }

                let child_element = convert_element_to_structured(&child, max_depth - 1);
                ui_element.children.push(child_element);
            }

            // Log completion of child processing
            if !children.is_empty() {
                crate::bprintln!(dev: "🖥️ XML_HELPER: Completed processing {} children for {} element", 
                                 children.len(), element_type);
            }
        }
    }

    ui_element
}
