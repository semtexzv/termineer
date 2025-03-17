//! Type definitions for the workflow system
//!
//! Defines the structure of workflows, steps, and parameters
//! using a field-based approach for step types.

use serde::Deserialize;
use serde_yaml;
use std::fmt;

/// A complete workflow definition
#[derive(Debug, Deserialize, Clone)]
pub struct Workflow {
    /// Name of the workflow
    pub name: String,

    /// Optional description of the workflow
    pub description: Option<String>,

    /// Optional version of the workflow
    #[allow(dead_code)]
    pub version: Option<String>,

    /// Optional author of the workflow
    #[allow(dead_code)]
    pub author: Option<String>,

    /// Parameters that can be passed to the workflow
    #[serde(default)]
    pub parameters: Vec<Parameter>,

    /// Optional default query template (can be overridden at runtime)
    pub query_template: Option<String>,

    /// Steps to execute in sequence
    pub steps: Vec<Step>,
}

/// A parameter for a workflow
#[derive(Debug, Deserialize, Clone)]
pub struct Parameter {
    /// Name of the parameter
    pub name: String,

    /// Optional description of the parameter
    #[allow(dead_code)]
    pub description: Option<String>,

    /// Type of the parameter (string, integer, etc.)
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub type_: String,

    /// Whether the parameter is required
    #[serde(default)]
    pub required: bool,

    /// Optional default value for the parameter
    #[allow(dead_code)]
    pub default: Option<serde_yaml::Value>,
}

/// A step in a workflow using a field-based approach
#[derive(Debug, Deserialize, Clone)]
pub struct Step {
    /// Common fields
    pub description: Option<String>,

    /// Step type identifiers
    #[serde(rename = "shell")]
    pub shell_id: Option<String>,

    #[serde(rename = "agent")]
    pub agent_id: Option<String>,

    /// Shell step fields
    pub command: Option<String>,
    pub store_output: Option<String>,
    #[allow(dead_code)]
    pub fail_on_error: Option<bool>,

    /// Agent step fields
    pub kind: Option<String>,
    pub prompt: Option<String>,
    pub into: Option<String>,

    /// Keep fields for message, file, output, and wait steps to maintain deserializing
    /// compatibility with existing workflow files, even though we don't use them
    #[serde(rename = "message")]
    #[allow(dead_code)]
    pub message_id: Option<String>,

    #[serde(rename = "file")]
    #[allow(dead_code)]
    pub file_id: Option<String>,

    #[serde(rename = "output")]
    #[allow(dead_code)]
    pub output_id: Option<String>,

    #[serde(rename = "wait")]
    #[allow(dead_code)]
    pub wait_id: Option<String>,

    #[allow(dead_code)]
    pub content: Option<String>,
    #[allow(dead_code)]
    pub store_response: Option<String>,
    #[allow(dead_code)]
    pub action: Option<FileAction>,
    #[allow(dead_code)]
    pub path: Option<String>,
    #[allow(dead_code)]
    pub wait_message: Option<String>,
}

/// The type of a workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// A shell command
    Shell,

    /// Agent step that creates a new agent
    Agent,

    /// Unknown step type
    Unknown,
}

impl fmt::Display for StepType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StepType::Shell => write!(f, "shell"),
            StepType::Agent => write!(f, "agent"),
            StepType::Unknown => write!(f, "unknown"),
        }
    }
}

impl Step {
    /// Get the type of this step
    pub fn get_type(&self) -> StepType {
        if self.shell_id.is_some() {
            StepType::Shell
        } else if self.agent_id.is_some() {
            StepType::Agent
        } else {
            StepType::Unknown
        }
    }

    /// Get the ID of this step regardless of type
    pub fn get_id(&self) -> String {
        if let Some(id) = &self.shell_id {
            id.clone()
        } else if let Some(id) = &self.agent_id {
            id.clone()
        } else {
            "unknown".to_string()
        }
    }
}

/// The action to perform on a file
/// Note: Kept for deserialization compatibility with existing workflow files
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileAction {
    /// Read a file
    Read,

    /// Write to a file
    Write,

    /// Append to a file
    Append,
}
