//! Context for workflow execution
//!
//! Handles variables, parameter storage, and template rendering
//! for workflow execution.

use handlebars::Handlebars;
use serde_json::{json, Value as JsonValue};
use serde_yaml;
use std::collections::HashMap;

use crate::workflow::types::Workflow;

/// Error types for workflow operations
#[derive(Debug, thiserror::Error)]
pub enum WorkflowError {
    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid step type")]
    InvalidStepType,

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Template error: {0}")]
    TemplateError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Shell command failed: {0}")]
    ShellError(String),

    #[error("Agent error: {0}")]
    AgentError(String),

    #[error("Invalid workflow configuration: {0}")]
    InvalidConfig(String),

    #[error("YAML error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

/// Context for workflow execution
pub struct WorkflowContext {
    /// Parameters passed to the workflow
    parameters: HashMap<String, serde_yaml::Value>,

    /// Variables set during workflow execution
    variables: HashMap<String, String>,

    /// Latest agent response
    agent_response: Option<String>,

    /// Query provided to the workflow
    query: Option<String>,
}

impl WorkflowContext {
    /// Create a new workflow context with the given parameters and query
    pub fn new(parameters: HashMap<String, serde_yaml::Value>, query: Option<String>) -> Self {
        Self {
            parameters,
            variables: HashMap::new(),
            agent_response: None,
            query,
        }
    }

    /// Set the query
    pub fn set_query(&mut self, query: Option<String>) {
        self.query = query;
    }

    /// Get the query
    #[allow(dead_code)]
    pub fn get_query(&self) -> Option<&String> {
        self.query.as_ref()
    }

    /// Validate that all required parameters are present
    pub fn validate_parameters(&self, workflow: &Workflow) -> Result<(), WorkflowError> {
        for param in &workflow.parameters {
            if param.required && !self.parameters.contains_key(&param.name) {
                return Err(WorkflowError::MissingParameter(param.name.clone()));
            }
        }

        Ok(())
    }

    /// Get a variable value
    #[allow(dead_code)]
    pub fn get_variable(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    /// Set a variable value
    pub fn set_variable(&mut self, name: String, value: String) {
        self.variables.insert(name, value);
    }

    /// Set the agent response
    #[allow(dead_code)]
    pub fn set_agent_response(&mut self, response: String) {
        self.agent_response = Some(response);
    }

    /// Get the agent response
    #[allow(dead_code)]
    pub fn get_agent_response(&self) -> Option<&String> {
        self.agent_response.as_ref()
    }

    /// Render a template with variable interpolation
    pub fn render_template(&self, template: &str) -> Result<String, WorkflowError> {
        let handlebars = Handlebars::new();

        // Create a combined context with parameters and variables
        let mut combined_context = HashMap::new();

        // Add parameters section
        let mut params_map = serde_json::Map::new();
        for (key, value) in &self.parameters {
            params_map.insert(key.clone(), self.yaml_to_json(value.clone()));
        }
        combined_context.insert("parameters".to_string(), json!(params_map));

        // Add individual variables
        for (key, value) in &self.variables {
            combined_context.insert(key.clone(), json!(value));
        }

        // Add agent response if available
        if let Some(response) = &self.agent_response {
            combined_context.insert("agent_response".to_string(), json!(response));
        }

        // Add query if available
        if let Some(query) = &self.query {
            combined_context.insert("query".to_string(), json!(query));
        }

        // Render the template
        handlebars
            .render_template(template, &combined_context)
            .map_err(|e| WorkflowError::TemplateError(e.to_string()))
    }

    /// Convert a YAML value to a JSON value for template rendering
    fn yaml_to_json(&self, yaml: serde_yaml::Value) -> JsonValue {
        match yaml {
            serde_yaml::Value::Null => JsonValue::Null,
            serde_yaml::Value::Bool(b) => JsonValue::Bool(b),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    JsonValue::Number(serde_json::Number::from(i))
                } else if let Some(f) = n.as_f64() {
                    serde_json::Number::from_f64(f)
                        .map(JsonValue::Number)
                        .unwrap_or(JsonValue::Null)
                } else {
                    JsonValue::Null
                }
            }
            serde_yaml::Value::String(s) => JsonValue::String(s),
            serde_yaml::Value::Sequence(seq) => {
                JsonValue::Array(seq.into_iter().map(|v| self.yaml_to_json(v)).collect())
            }
            serde_yaml::Value::Mapping(map) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in map {
                    if let serde_yaml::Value::String(key) = k {
                        obj.insert(key, self.yaml_to_json(v));
                    }
                }
                JsonValue::Object(obj)
            }
            _ => JsonValue::Null,
        }
    }
}
