//! Content types for MCP protocol messages

use crate::llm::ImageSource;
use serde::{Deserialize, Serialize};

/// Base trait for MCP content types
pub trait McpContent {
    /// Convert to LLM content type
    fn to_llm_content(&self) -> crate::llm::Content;
}

/// Content type that can be sent to or from an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Content {
    /// Text content
    #[serde(rename = "text")]
    Text(TextContent),

    /// Image content
    #[serde(rename = "image")]
    Image(ImageContent),

    /// Embedded resource
    #[serde(rename = "resource")]
    Resource(EmbeddedResource),
}

impl McpContent for Content {
    fn to_llm_content(&self) -> crate::llm::Content {
        match self {
            Content::Text(text) => text.to_llm_content(),
            Content::Image(image) => image.to_llm_content(),
            Content::Resource(resource) => resource.to_llm_content(),
        }
    }
}

/// Text content provided to or from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextContent {
    /// The text content
    pub text: String,

    /// Optional annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

impl McpContent for TextContent {
    fn to_llm_content(&self) -> crate::llm::Content {
        crate::llm::Content::Text {
            text: self.text.clone(),
        }
    }
}

/// Image content provided to or from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageContent {
    /// The base64-encoded image data
    pub data: String,

    /// The MIME type of the image
    pub mime_type: String,

    /// Optional annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

impl McpContent for ImageContent {
    fn to_llm_content(&self) -> crate::llm::Content {
        // Convert to our internal image format
        // Currently we just store the source which should be a data URI
        crate::llm::Content::Image {
            source: ImageSource::Base64 {
                data: self.data.clone(),
                media_type: self.mime_type.clone(),
            },
        }
    }
}

/// Resource contents that can be text or binary
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceContents {
    /// Text resource
    Text(TextResourceContents),

    /// Binary resource
    Binary(BlobResourceContents),
}

/// Text resource contents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextResourceContents {
    /// The URI of this resource
    pub uri: String,

    /// The text content
    pub text: String,

    /// The MIME type if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Binary resource contents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContents {
    /// The URI of this resource
    pub uri: String,

    /// Base64-encoded binary data
    pub blob: String,

    /// The MIME type if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Embedded resource in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResource {
    /// The resource contents
    pub resource: ResourceContents,

    /// Optional annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Annotations>,
}

impl McpContent for EmbeddedResource {
    fn to_llm_content(&self) -> crate::llm::Content {
        match &self.resource {
            ResourceContents::Text(text) => crate::llm::Content::Text {
                text: format!("Resource {}: {}", text.uri, text.text),
            },
            ResourceContents::Binary(blob) => {
                let mime_type = blob
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                if mime_type.starts_with("image/") {
                    crate::llm::Content::Image {
                        source: ImageSource::Base64 {
                            media_type: mime_type,
                            data: blob.blob.clone(),
                        },
                    }
                } else {
                    crate::llm::Content::Document {
                        source: blob.uri.clone(),
                    }
                }
            }
        }
    }
}

/// Annotations for MCP content
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
