//! Tool-related structures for MCP protocol

use serde::{Deserialize, Serialize};

/// Represents a tool provided by an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value, // JSON Schema for tool input
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
    pub content: Vec<serde_json::Value>,
    #[serde(default)]
    pub is_error: bool,
}

impl CallToolResult {
    /// Convert tool result to Content objects
    pub fn to_content_objects(&self) -> anyhow::Result<Vec<super::content::Content>> {
        let mut contents = Vec::new();

        for item in &self.content {
            // Try to parse as Content enum first
            if let Ok(content) = serde_json::from_value::<super::content::Content>(item.clone()) {
                contents.push(content);
                continue;
            }

            // If that fails, check if it's a simple text object
            if let Some(text) = item.as_str() {
                // Create a text content object
                contents.push(super::content::Content::Text(super::content::TextContent {
                    type_id: "text".to_string(),
                    text: text.to_string(),
                    annotations: None,
                }));
                continue;
            }

            // Otherwise, convert to string and make it text content
            contents.push(super::content::Content::Text(super::content::TextContent {
                type_id: "text".to_string(),
                text: serde_json::to_string_pretty(item)?,
                annotations: None,
            }));
        }

        Ok(contents)
    }
}
