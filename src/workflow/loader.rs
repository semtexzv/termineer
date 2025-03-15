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

/// Get path to the home directory workflow location
fn get_home_workflows_path() -> Option<PathBuf> {
    home_dir().map(|path| path.join(".termineer").join("workflows"))
}

/// Get path to the local workflows directory
fn get_local_workflows_path() -> PathBuf {
    PathBuf::from(".termineer").join("workflows")
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
    
    let alt_name = if normalized_name.ends_with(".yaml") {
        normalized_name.replace(".yaml", ".yml")
    } else {
        normalized_name.replace(".yml", ".yaml")
    };
    
    // Check in the current directory first
    let local_dir = get_local_workflows_path();
    
    // Try with original extension
    let local_path = local_dir.join(&normalized_name);
    if local_path.exists() {
        println!("Found workflow in local directory: {}", local_path.display());
        return Ok(local_path);
    }
    
    // Try with alternate extension
    let alt_local_path = local_dir.join(&alt_name);
    if alt_local_path.exists() {
        println!("Found workflow in local directory: {}", alt_local_path.display());
        return Ok(alt_local_path);
    }
    
    // Check in the user's home directory
    if let Some(global_dir) = get_home_workflows_path() {
        // Try with original extension
        let global_path = global_dir.join(&normalized_name);
        if global_path.exists() {
            println!("Found workflow in home directory: {}", global_path.display());
            return Ok(global_path);
        }
        
        // Try with alternate extension
        let alt_global_path = global_dir.join(&alt_name);
        if alt_global_path.exists() {
            println!("Found workflow in home directory: {}", alt_global_path.display());
            return Ok(alt_global_path);
        }
    }
    
    // Workflow file not found
    Err(WorkflowError::InvalidConfig(format!(
        "Workflow file not found: {} (searched in local and home .termineer/workflows/ directories)",
        name
    )))
}

/// Ensure the workflows directory exists
///
/// Creates the .termineer/workflows directory if it doesn't exist,
/// both in the current directory and in the user's home directory.
#[allow(dead_code)]
pub fn ensure_workflows_directory() -> Result<(), WorkflowError> {
    // Create local directory
    let local_dir = get_local_workflows_path();
    if !local_dir.exists() {
        println!("Creating local workflows directory: {}", local_dir.display());
        fs::create_dir_all(&local_dir)
            .map_err(|e| WorkflowError::IoError(e))?;
    }
    
    // Create global directory
    if let Some(global_dir) = get_home_workflows_path() {
        if !global_dir.exists() {
            println!("Creating home workflows directory: {}", global_dir.display());
            fs::create_dir_all(&global_dir)
                .map_err(|e| WorkflowError::IoError(e))?;
        }
    }
    
    Ok(())
}

/// List all available workflows
///
/// Returns a list of all workflow names (without extensions)
/// found in the .termineer/workflows directories.
#[allow(dead_code)]
pub fn list_workflows() -> Result<Vec<String>, WorkflowError> {
    let mut workflows = Vec::new();
    
    // Check local directory
    let local_dir = get_local_workflows_path();
    if local_dir.exists() {
        add_workflows_from_dir(&local_dir, &mut workflows)?;
    }
    
    // Check global directory
    if let Some(global_dir) = get_home_workflows_path() {
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
#[allow(dead_code)]
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