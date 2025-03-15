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
    pub version: Option<String>,
    
    /// Optional author of the workflow
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
    pub description: Option<String>,
    
    /// Type of the parameter (string, integer, etc.)
    #[serde(rename = "type")]
    pub type_: String,
    
    /// Whether the parameter is required
    #[serde(default)]
    pub required: bool,
    
    /// Optional default value for the parameter
    pub default: Option<serde_yaml::Value>,
}

/// A step in a workflow using a field-based approach
#[derive(Debug, Deserialize, Clone)]
pub struct Step {
    /// Common fields
    pub description: Option<String>,
    
    /// Only one of these will be present, determining the step type
    #[serde(rename = "shell")]
    pub shell_id: Option<String>,
    
    #[serde(rename = "message")]
    pub message_id: Option<String>,
    
    #[serde(rename = "file")]
    pub file_id: Option<String>,
    
    #[serde(rename = "output")]
    pub output_id: Option<String>,
    
    #[serde(rename = "wait")]
    pub wait_id: Option<String>,
    
    /// Shell step fields
    pub command: Option<String>,
    pub store_output: Option<String>,
    pub fail_on_error: Option<bool>,
    
    /// Message step fields (for agent interaction)
    pub content: Option<String>,
    pub store_response: Option<String>,
    
    /// File step fields
    pub action: Option<FileAction>,
    pub path: Option<String>,
    
    /// Wait step fields (separate from message to avoid conflict with message_id)
    pub wait_message: Option<String>,
}

/// The type of a workflow step
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepType {
    /// A shell command
    Shell,
    
    /// A message to the agent
    Message,
    
    /// A file operation
    File,
    
    /// Output to the user
    Output,
    
    /// Wait for user input
    Wait,
    
    /// Unknown step type
    Unknown,
}

impl fmt::Display for StepType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StepType::Shell => write!(f, "shell"),
            StepType::Message => write!(f, "message"),
            StepType::File => write!(f, "file"),
            StepType::Output => write!(f, "output"),
            StepType::Wait => write!(f, "wait"),
            StepType::Unknown => write!(f, "unknown"),
        }
    }
}

impl Step {
    /// Get the type of this step
    pub fn get_type(&self) -> StepType {
        if self.shell_id.is_some() {
            StepType::Shell
        } else if self.message_id.is_some() {
            StepType::Message
        } else if self.file_id.is_some() {
            StepType::File
        } else if self.output_id.is_some() {
            StepType::Output
        } else if self.wait_id.is_some() {
            StepType::Wait
        } else {
            StepType::Unknown
        }
    }
    
    /// Get the ID of this step regardless of type
    pub fn get_id(&self) -> String {
        if let Some(id) = &self.shell_id {
            id.clone()
        } else if let Some(id) = &self.message_id {
            id.clone()
        } else if let Some(id) = &self.file_id {
            id.clone()
        } else if let Some(id) = &self.output_id {
            id.clone()
        } else if let Some(id) = &self.wait_id {
            id.clone()
        } else {
            "unknown".to_string()
        }
    }
}

/// The action to perform on a file
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