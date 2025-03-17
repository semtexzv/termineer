//! Tool-related structures for MCP protocol

use serde::{Deserialize, Serialize};

/// Represents a tool provided by an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: super::schema::JsonSchema, // JSON Schema for tool input
}

/// Parameters for a ListToolsRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Response from a ListToolsRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

/// Parameters for a CallToolRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolParams {
    pub name: String,

    pub arguments: serde_json::Value,
}

/// Response from a CallToolRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    // Main format - 2024-11-05
    pub content: Vec<serde_json::Value>,
    #[serde(default)]
    pub is_error: bool,

    // Legacy format - 2024-10-07 backward compatibility
    // Will only be used when deserializing, never when serializing
    #[serde(skip_serializing)]
    pub tool_result: Option<serde_json::Value>,
}

impl CallToolResult {
    /// Convert tool result to Content objects
    pub fn to_content_objects(&self) -> anyhow::Result<Vec<super::content::Content>> {
        let mut contents = Vec::new();

        // If we have content (current format), use that
        if !self.content.is_empty() {
            for item in &self.content {
                bprintln!(dev: "ITEM: {:#?}", item);
                // Try to parse as Content enum first
                if let Ok(content) = serde_json::from_value::<super::content::Content>(item.clone())
                {
                    contents.push(content);
                    continue;
                }

                // If that fails, check if it's a simple text object
                if let Some(text) = item.as_str() {
                    // Create a text content object
                    contents.push(super::content::Content::Text(super::content::TextContent {
                        text: text.to_string(),
                        annotations: None,
                    }));
                    continue;
                }
                bprintln!(warn: "Unknown content type: {:#?}", item.get("type"));
                // Otherwise, convert to string and make it text content
                contents.push(super::content::Content::Text(super::content::TextContent {
                    text: serde_json::to_string_pretty(item)?,
                    annotations: None,
                }));
            }
        }
        // Handle legacy format (2024-10-07) if content is empty but tool_result exists
        else if let Some(tool_result) = &self.tool_result {
            bprintln!(dev: "Using legacy tool_result format: {:#?}", tool_result);
            // Convert the legacy tool_result to a text content
            contents.push(super::content::Content::Text(super::content::TextContent {
                text: match tool_result {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string_pretty(tool_result)?,
                },
                annotations: None,
            }));
        }

        Ok(contents)
    }
}
