//! CLI integration for the workflow system
//!
//! Provides functions for parsing command line arguments and
//! executing workflows from the CLI.

use std::collections::HashMap;
use anyhow::format_err;
use serde_yaml::Value as YamlValue;

use crate::workflow::loader::{ensure_workflows_directory, list_workflows, load_workflow};
use crate::workflow::executor;
use crate::workflow::context::WorkflowError;

/// Handle the workflow command from the CLI
pub async fn handle_workflow_command(
    name: &Option<String>,
    param_values: &[String],
    query: Option<String>,
    agent_id: crate::agent::AgentId,
) -> anyhow::Result<()> {
    // Ensure the workflows directory exists
    ensure_workflows_directory()?;
    
    // If no workflow name is provided, list available workflows
    if name.is_none() {
        return list_available_workflows().await;
    }
    
    // Get the workflow name
    let workflow_name = name.as_ref().ok_or(format_err!("No workflow specified"))?;
    
    // Load the workflow
    let workflow = load_workflow(workflow_name)?;
    
    // Parse parameters
    let parameters = parse_parameters_from_values(param_values, &workflow)?;
    
    // Execute the workflow
    executor::execute_workflow(&workflow, parameters, query, agent_id).await?;
    
    Ok(())
}

/// List all available workflows
async fn list_available_workflows() -> anyhow::Result<()> {
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