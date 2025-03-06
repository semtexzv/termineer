//! Common types for MCP protocol

use serde::{Deserialize, Serialize};

/// Base for objects that include optional annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotated {
    /// Optional annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

/// Annotations for MCP objects
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotations {
    /// The priority of this content (0-1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f32>,

    /// The audience for this content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
}
