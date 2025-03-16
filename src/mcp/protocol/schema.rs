//! JSON Schema related structures and helpers for MCP protocol

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JSON Schema primitive type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SchemaType {
    String,
    Number,
    Integer,
    Boolean,
    Array,
    Object,
    Null,
    #[serde(other)]
    Any,
}

impl SchemaType {
    /// Convert the type to a string representation
    #[allow(dead_code)]
    pub fn to_string(&self) -> String {
        match self {
            SchemaType::String => "string".to_string(),
            SchemaType::Number => "number".to_string(),
            SchemaType::Integer => "integer".to_string(),
            SchemaType::Boolean => "boolean".to_string(),
            SchemaType::Array => "array".to_string(),
            SchemaType::Object => "object".to_string(),
            SchemaType::Null => "null".to_string(),
            SchemaType::Any => "any".to_string(),
        }
    }
}

/// Value for examples, defaults, or enum options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum SchemaValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Null,
    Array(Vec<SchemaValue>),
    Object(HashMap<String, SchemaValue>),
}

impl SchemaValue {
    /// Format the value as a string suitable for example display
    #[allow(dead_code)]
    pub fn format(&self) -> String {
        match self {
            SchemaValue::String(s) => format!("\"{}\"", s),
            SchemaValue::Number(n) => n.to_string(),
            SchemaValue::Integer(i) => i.to_string(),
            SchemaValue::Boolean(b) => b.to_string(),
            SchemaValue::Null => "null".to_string(),
            SchemaValue::Array(arr) => {
                if arr.is_empty() {
                    "[]".to_string()
                } else {
                    let items: Vec<String> = arr.iter().take(3).map(|v| v.format()).collect();
                    if arr.len() <= 3 {
                        format!("[{}]", items.join(", "))
                    } else {
                        format!("[{}, ...]", items.join(", "))
                    }
                }
            },
            SchemaValue::Object(_) => "{ ... }".to_string(),
        }
    }
}

/// Represents a JSON Schema object for tool input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    /// JSON Schema type (usually "object" for tool inputs)
    #[serde(rename = "type")]
    pub schema_type: Option<SchemaType>,
    
    /// Properties of the schema (for "object" types)
    pub properties: Option<HashMap<String, PropertySchema>>,
    
    /// Required property names (for "object" types)
    pub required: Option<Vec<String>>,
    
    /// Additional properties allowed flag
    #[serde(rename = "additionalProperties")]
    pub additional_properties: Option<bool>,
    
    /// Schema description
    pub description: Option<String>,
}

/// Represents a property schema in a JSON Schema object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    /// Property type
    #[serde(rename = "type")]
    pub property_type: Option<SchemaType>,
    
    /// Property description
    pub description: Option<String>,
    
    /// Format (for string types: date, uri, etc.)
    pub format: Option<String>,
    
    /// Pattern (for string validation)
    pub pattern: Option<String>,
    
    /// Enum values (for enumeration types)
    #[serde(rename = "enum")]
    pub enum_values: Option<Vec<SchemaValue>>,
    
    /// Example value
    pub example: Option<SchemaValue>,
    
    /// Default value
    pub default: Option<SchemaValue>,
    
    /// Minimum value (for numeric types)
    pub minimum: Option<f64>,
    
    /// Maximum value (for numeric types)
    pub maximum: Option<f64>,
    
    /// Items schema (for array types)
    pub items: Option<Box<PropertySchema>>,
}

impl PropertySchema {
    /// Get the type as a string
    #[allow(dead_code)]
    pub fn type_string(&self) -> String {
        self.property_type
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "any".to_string())
    }
    
    /// Generate an example value string for this property
    #[allow(dead_code)]
    pub fn example_value(&self) -> String {
        // Use example if available
        if let Some(example) = &self.example {
            return example.format();
        }
        
        // Use default if available
        if let Some(default) = &self.default {
            return default.format();
        }
        
        // Use first enum value if available
        if let Some(enum_values) = &self.enum_values {
            if !enum_values.is_empty() {
                return enum_values[0].format();
            }
        }
        
        // Generate based on type and format
        match self.property_type.as_ref().unwrap_or(&SchemaType::Any) {
            SchemaType::String => {
                if let Some(format) = &self.format {
                    match format.as_str() {
                        "date" => "\"2023-01-01\"".to_string(),
                        "date-time" => "\"2023-01-01T12:00:00Z\"".to_string(),
                        "email" => "\"user@example.com\"".to_string(),
                        "uri" => "\"https://example.com\"".to_string(),
                        _ => "\"example\"".to_string(),
                    }
                } else {
                    "\"example\"".to_string()
                }
            },
            SchemaType::Number => "3.14".to_string(),
            SchemaType::Integer => "42".to_string(),
            SchemaType::Boolean => "true".to_string(),
            SchemaType::Array => {
                if let Some(items) = &self.items {
                    format!("[{}]", items.example_value())
                } else {
                    "[]".to_string()
                }
            },
            SchemaType::Object => "{ ... }".to_string(),
            SchemaType::Null => "null".to_string(),
            SchemaType::Any => "\"example\"".to_string(),
        }
    }
}