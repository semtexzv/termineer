//! Loader for workflow definitions
//!
//! Handles loading workflow definitions from YAML files
//! in the `.termineer/workflows` directory.

use std::fs;
use std::path::{Path, PathBuf};
use crate::workflow::types::Workflow;
use crate::workflow::context::WorkflowError;
use dirs::home_dir;

/// Load a workflow by name
///
/// Looks for a workflow file in the `.termineer/workflows` directory
/// with the given name (with or without .yaml extension).
pub fn load_workflow(name: &str) -> Result<Workflow, WorkflowError> {
    // Find the workflow file
    let workflow_path = find_workflow_file(name)?;
    
    // Read the file content
    let content = fs::read_to_string(&workflow_path)
        .map_err(|e| WorkflowError::IoError(e))?;
    
    // Parse the YAML content
    let workflow: Workflow = serde_yaml::from_str(&content)?;
    
    Ok(workflow)
}

/// Find a workflow file by name
///
/// Searches in the following locations:
/// 1. .termineer/workflows/ in the current directory
/// 2. .termineer/workflows/ in the user's home directory
fn find_workflow_file(name: &str) -> Result<PathBuf, WorkflowError> {
    // Normalize the name to ensure it has a .yaml extension
    let normalized_name = if name.ends_with(".yaml") || name.ends_with(".yml") {
        name.to_string()
    } else {
        format!("{}.yaml", name)
    };
    
    // Check in the current directory first
    let local_dir = Path::new(".termineer/workflows");
    let local_path = local_dir.join(&normalized_name);
    if local_path.exists() {
        return Ok(local_path);
    }
    
    // Check for the alternate extension
    if normalized_name.ends_with(".yaml") {
        let alt_name = normalized_name.replace(".yaml", ".yml");
        let alt_path = local_dir.join(alt_name);
        if alt_path.exists() {
            return Ok(alt_path);
        }
    } else if normalized_name.ends_with(".yml") {
        let alt_name = normalized_name.replace(".yml", ".yaml");
        let alt_path = local_dir.join(alt_name);
        if alt_path.exists() {
            return Ok(alt_path);
        }
    }
    
    // Check in the user's home directory
    if let Some(home) = home_dir() {
        let global_dir = home.join(".termineer/workflows");
        let global_path = global_dir.join(&normalized_name);
        if global_path.exists() {
            return Ok(global_path);
        }
        
        // Check for the alternate extension
        if normalized_name.ends_with(".yaml") {
            let alt_name = normalized_name.replace(".yaml", ".yml");
            let alt_path = global_dir.join(alt_name);
            if alt_path.exists() {
                return Ok(alt_path);
            }
        } else if normalized_name.ends_with(".yml") {
            let alt_name = normalized_name.replace(".yml", ".yaml");
            let alt_path = global_dir.join(alt_name);
            if alt_path.exists() {
                return Ok(alt_path);
            }
        }
    }
    
    // Workflow file not found
    Err(WorkflowError::InvalidConfig(format!(
        "Workflow file not found: {} (searched in .termineer/workflows/)",
        name
    )))
}

/// Ensure the workflows directory exists
///
/// Creates the .termineer/workflows directory if it doesn't exist,
/// both in the current directory and in the user's home directory.
pub fn ensure_workflows_directory() -> Result<(), WorkflowError> {
    // Create local directory
    let local_dir = Path::new(".termineer/workflows");
    if !local_dir.exists() {
        fs::create_dir_all(local_dir)
            .map_err(|e| WorkflowError::IoError(e))?;
    }
    
    // Create global directory
    if let Some(home) = home_dir() {
        let global_dir = home.join(".termineer/workflows");
        if !global_dir.exists() {
            fs::create_dir_all(global_dir)
                .map_err(|e| WorkflowError::IoError(e))?;
        }
    }
    
    Ok(())
}

/// List all available workflows
///
/// Returns a list of all workflow names (without extensions)
/// found in the .termineer/workflows directories.
pub fn list_workflows() -> Result<Vec<String>, WorkflowError> {
    let mut workflows = Vec::new();
    
    // Check local directory
    let local_dir = Path::new(".termineer/workflows");
    if local_dir.exists() {
        add_workflows_from_dir(local_dir, &mut workflows)?;
    }
    
    // Check global directory
    if let Some(home) = home_dir() {
        let global_dir = home.join(".termineer/workflows");
        if global_dir.exists() {
            add_workflows_from_dir(&global_dir, &mut workflows)?;
        }
    }
    
    // Sort and deduplicate
    workflows.sort();
    workflows.dedup();
    
    Ok(workflows)
}

/// Add workflows from a directory to the list
fn add_workflows_from_dir(dir: &Path, workflows: &mut Vec<String>) -> Result<(), WorkflowError> {
    for entry in fs::read_dir(dir).map_err(|e| WorkflowError::IoError(e))? {
        let entry = entry.map_err(|e| WorkflowError::IoError(e))?;
        let path = entry.path();
        
        if let Some(ext) = path.extension() {
            if ext == "yaml" || ext == "yml" {
                if let Some(name) = path.file_stem() {
                    if let Some(name_str) = name.to_str() {
                        workflows.push(name_str.to_string());
                    }
                }
            }
        }
    }
    
    Ok(())
}