//! Tool-related structures for MCP protocol

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Represents a tool provided by an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,  // JSON Schema for tool input
    #[serde(rename = "outputSchema")]
    pub output_schema: Option<serde_json::Value>,  // JSON Schema for tool output
    #[serde(rename = "_meta")]
    pub meta: Option<HashMap<String, serde_json::Value>>,
}

/// Parameters for a ListToolsRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

/// Response from a ListToolsRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResult {
    pub tools: Vec<Tool>,
}

/// Parameters for a CallToolRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    pub id: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

/// Response from a CallToolRequest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolResult {
    pub output: serde_json::Value,
}