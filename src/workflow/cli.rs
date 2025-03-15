//! CLI integration for the workflow system
//!
//! Provides functions for parsing command line arguments and
//! executing workflows from the CLI.

use std::collections::HashMap;
use anyhow::{format_err, Result};
use serde_yaml::Value as YamlValue;

use crate::workflow::loader::{ensure_workflows_directory, list_workflows, load_workflow};
use crate::workflow::executor;
use crate::workflow::context::WorkflowError;

/// Handle the workflow command from the CLI

/// List all available workflows
async fn list_available_workflows() -> anyhow::Result<()> {
    // Check if user has Pro access - workflows are a Pro-only feature
    if crate::config::get_app_mode() != crate::config::AppMode::Pro {
        return Err(format_err!("Workflows are a Pro-only feature. Upgrade to Pro for access."));
    }
    
    let workflows = list_workflows()?;
    
    if workflows.is_empty() {
        println!("No workflows found. Create one in .termineer/workflows/");
        return Ok(());
    }
    
    println!("Available workflows:");
    for workflow in workflows {
        println!("  - {}", workflow);
    }
    
    println!("\nRun with: termineer workflow <name>");
    Ok(())
}

/// Parse parameters from a list of key=value strings
fn parse_parameters_from_values(
    param_values: &[String],
    workflow: &crate::workflow::types::Workflow,
) -> Result<HashMap<String, YamlValue>, WorkflowError> {
    let mut parameters = HashMap::new();
    
    // Get parameters from command line
    for param in param_values {
        // Parse key=value format
        if let Some((key, value)) = param.split_once('=') {
            parameters.insert(key.to_string(), YamlValue::String(value.to_string()));
        } else {
            return Err(WorkflowError::InvalidConfig(format!(
                "Invalid parameter format: {}. Use key=value",
                param
            )));
        }
    }
    
    // Fill in defaults for parameters not provided
    for param in &workflow.parameters {
        if !parameters.contains_key(&param.name) {
            if let Some(default) = &param.default {
                parameters.insert(param.name.clone(), default.clone());
            } else if param.required {
                return Err(WorkflowError::MissingParameter(param.name.clone()));
            }
        }
    }
    
    Ok(parameters)
}