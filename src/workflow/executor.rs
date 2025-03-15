//! Executor for workflow steps
//!
//! Handles executing each step in a workflow, managing the context,
//! and coordinating between steps.

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::process::Command;
use std::io::{self, Write};
use tokio::fs;
use tokio::time::Duration;
use tokio::io::AsyncWriteExt;

use crate::agent::{AgentManager, AgentMessage, AgentState};
use crate::workflow::types::{Workflow, Step, StepType, FileAction};
use crate::workflow::context::{WorkflowContext, WorkflowError};

/// Executor for workflows
pub struct WorkflowExecutor {
    /// The agent manager
    agent_manager: Arc<Mutex<crate::agent::AgentManager>>,
    
    /// The agent ID to use for executing messages
    agent_id: crate::agent::AgentId,
}

impl WorkflowExecutor {
    /// Create a new workflow executor with the given agent
    pub fn new(agent_manager: Arc<Mutex<crate::agent::AgentManager>>, agent_id: crate::agent::AgentId) -> Self {
        Self { agent_manager, agent_id }
    }
    
    /// Execute a workflow with the given parameters and optional query
    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
        parameters: HashMap<String, serde_yaml::Value>,
        query: Option<String>,
    ) -> Result<(), WorkflowError> {
        // Create workflow context
        let mut context = WorkflowContext::new(parameters, query.clone());
        
        // Process query template if available
        if let Some(query_text) = query {
            if let Some(template) = &workflow.query_template {
                // If the workflow has a query template, render it with the query as a variable
                // This allows workflows to add structure around the user's query
                context.set_variable("raw_query".to_string(), query_text);
                let rendered_query = context.render_template(template)?;
                context.set_query(Some(rendered_query));
            }
        }
        
        // Validate parameters
        context.validate_parameters(workflow)?;
        
        // Log workflow start
        println!("Starting workflow: {} - {}", 
                 workflow.name, 
                 workflow.description.as_deref().unwrap_or(""));
        
        // Execute each step sequentially
        for (step_index, step) in workflow.steps.iter().enumerate() {
            let step_type = step.get_type();
            let step_id = step.get_id();
            
            println!("Step {}/{}: {} - {}", 
                     step_index + 1, 
                     workflow.steps.len(),
                     step_id, 
                     step.description.as_deref().unwrap_or(""));
            
            match step_type {
                StepType::Shell => {
                    self.execute_shell_step(step, &mut context).await?;
                },
                StepType::Message => {
                    self.execute_message_step(step, &mut context).await?;
                },
                StepType::File => {
                    self.execute_file_step(step, &mut context).await?;
                },
                StepType::Output => {
                    self.execute_output_step(step, &mut context).await?;
                },
                StepType::Wait => {
                    self.execute_wait_step(step, &mut context).await?;
                },
                StepType::Unknown => {
                    return Err(WorkflowError::InvalidStepType);
                }
            }
        }
        
        println!("Workflow completed successfully");
        Ok(())
    }
    
    /// Execute a shell command step
    async fn execute_shell_step(
        &self, 
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<(), WorkflowError> {
        // Verify required fields
        let command = step.command.as_ref()
            .ok_or(WorkflowError::MissingField("command".to_string()))?;
        
        // Render command with variable interpolation
        let rendered_command = context.render_template(command)?;
        
        // Execute shell command directly
        println!("Executing: {}", rendered_command);
        
        let output = self.execute_shell_command(&rendered_command)
            .map_err(|e| WorkflowError::ShellError(e.to_string()))?;
        
        // Store output if specified
        if let Some(var_name) = &step.store_output {
            context.set_variable(var_name.clone(), output);
        }
        
        Ok(())
    }
    
    /// Execute a shell command and return its output
    fn execute_shell_command(&self, command: &str) -> Result<String, io::Error> {
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(["/C", command])
                .output()?
        } else {
            Command::new("sh")
                .args(["-c", command])
                .output()?
        };
        
        // If command failed and we should fail on error
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            println!("Command failed with error: {}", stderr);
        }
        
        // Combine stdout and stderr
        let mut result = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.stderr.is_empty() {
            if !result.is_empty() {
                result.push_str("\n");
            }
            result.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(result)
    }
    
    /// Execute an agent message step
    async fn execute_message_step(
        &self, 
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<(), WorkflowError> {
        // Verify required fields
        let content = step.content.as_ref()
            .ok_or(WorkflowError::MissingField("content".to_string()))?;
        
        // Render message with variable interpolation
        let rendered_content = context.render_template(content)?;
        
        // Send to agent as a user input message
        println!("Sending message to agent: {}", step.get_id());
        
        // Create user input message
        let message = AgentMessage::UserInput(rendered_content);
        
        // Get the current state of the agent to detect when it changes
        let initial_state = {
            let manager = self.agent_manager.lock().unwrap();
            manager.get_agent_state(self.agent_id)
                .unwrap_or(AgentState::Idle)
        };
        
        // Send the message to the agent
        {
            let manager = self.agent_manager.lock().unwrap();
            manager.send_message(self.agent_id, message)
                .map_err(|e| WorkflowError::AgentError(format!("Failed to send message: {}", e)))?;
        }
        
        // Wait for the agent to process the message
        let mut attempts = 0;
        let max_attempts = 300; // Wait up to 5 minutes (1 second intervals)
        
        // Wait for the state to change from Processing back to Idle or Done
        let mut done_state: Option<AgentState> = None;
        
        while done_state.is_none() && attempts < max_attempts {
            // Sleep briefly to avoid busy-waiting
            tokio::time::sleep(Duration::from_secs(1)).await;
            
            // Get the agent's current state
            let current_state = {
                let manager = self.agent_manager.lock().unwrap();
                manager.get_agent_state(self.agent_id)
                    .unwrap_or(AgentState::Idle)
            };
            
            // Check if the state indicates completion
            match &current_state {
                AgentState::Idle => {
                    // State has changed to Idle, indicating completion
                    if let AgentState::Processing = initial_state {
                        done_state = Some(current_state);
                    }
                },
                AgentState::Done(response) => {
                    // State has changed to Done, with a response
                    done_state = Some(current_state);
                },
                AgentState::Processing => {
                    // Still processing, continue waiting
                },
                AgentState::RunningTool { .. } => {
                    // Tool is running, continue waiting
                },
                AgentState::Terminated => {
                    // Agent was terminated, error out
                    return Err(WorkflowError::AgentError("Agent was terminated".to_string()));
                },
            }
            
            attempts += 1;
        }
        
        if attempts >= max_attempts {
            return Err(WorkflowError::AgentError(
                "Timeout waiting for agent response".to_string()
            ));
        }
        
        // Extract the response from the agent's buffer
        let response = {
            let manager = self.agent_manager.lock().unwrap();
            
            // Get the buffer contents
            if let Ok(buffer) = manager.get_agent_buffer(self.agent_id) {
                // Extract the most recent assistant message
                // This is a simplified approach - in a real implementation,
                // we would parse the buffer more carefully
                let buffer_text = format!("{:?}", buffer);
                
                // Use a simple heuristic to extract the last response
                // In a real implementation, this would be more sophisticated
                buffer_text
            } else {
                "Could not retrieve agent response".to_string()
            }
        };
        
        // Store the agent's response in the context
        context.set_agent_response(response.clone());
        
        // Also store in a named variable if specified
        if let Some(var_name) = &step.store_response {
            context.set_variable(var_name.clone(), response);
        }
        
        Ok(())
    }
    
    /// Execute a file operation step
    async fn execute_file_step(
        &self, 
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<(), WorkflowError> {
        // Verify required fields
        let action = step.action.as_ref()
            .ok_or(WorkflowError::MissingField("action".to_string()))?;
        let path = step.path.as_ref()
            .ok_or(WorkflowError::MissingField("path".to_string()))?;
        
        // Render path with variable interpolation
        let rendered_path = context.render_template(path)?;
        
        match action {
            FileAction::Read => {
                // Read file content
                let content = fs::read_to_string(&rendered_path)
                    .await
                    .map_err(|e| WorkflowError::IoError(e))?;
                
                // Store content if specified
                if let Some(var_name) = &step.store_output {
                    context.set_variable(var_name.clone(), content);
                }
            },
            FileAction::Write => {
                // Check if content is provided
                if let Some(content_template) = &step.content {
                    // Render content with variable interpolation
                    let content = context.render_template(content_template)?;
                    
                    // Ensure the directory exists
                    if let Some(parent) = std::path::Path::new(&rendered_path).parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent)
                                .await
                                .map_err(|e| WorkflowError::IoError(e))?;
                        }
                    }
                    
                    // Write to file
                    fs::write(&rendered_path, content)
                        .await
                        .map_err(|e| WorkflowError::IoError(e))?;
                } else {
                    return Err(WorkflowError::MissingField("content".to_string()));
                }
            },
            FileAction::Append => {
                // Check if content is provided
                if let Some(content_template) = &step.content {
                    // Render content with variable interpolation
                    let content = context.render_template(content_template)?;
                    
                    // Ensure the directory exists
                    if let Some(parent) = std::path::Path::new(&rendered_path).parent() {
                        if !parent.exists() {
                            fs::create_dir_all(parent)
                                .await
                                .map_err(|e| WorkflowError::IoError(e))?;
                        }
                    }
                    
                    // Append to file (open in append mode)
                    let mut file = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&rendered_path)
                        .await
                        .map_err(|e| WorkflowError::IoError(e))?;
                    
                    file.write_all(content.as_bytes())
                        .await
                        .map_err(|e| WorkflowError::IoError(e))?;
                } else {
                    return Err(WorkflowError::MissingField("content".to_string()));
                }
            },
        }
        
        Ok(())
    }
    
    /// Execute an output step
    async fn execute_output_step(
        &self, 
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<(), WorkflowError> {
        // Verify required fields
        let content = step.content.as_ref()
            .ok_or(WorkflowError::MissingField("content".to_string()))?;
        
        // Render content with variable interpolation
        let rendered_content = context.render_template(content)?;
        
        // Format output with a prefix to make it stand out
        let formatted_output = format!("🔶 WORKFLOW OUTPUT: {}", rendered_content);
        
        // Print to console using buffer printing
        println!("{}", formatted_output);
        
        // Optionally, we could also send this output to the agent's buffer
        // for visualization in the UI, but this might be confusing
        
        Ok(())
    }
    
    /// Execute a wait step
    async fn execute_wait_step(
        &self, 
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<(), WorkflowError> {
        // Render message if provided, or use default
        let message = if let Some(msg_template) = &step.wait_message {
            context.render_template(msg_template)?
        } else {
            "Press Enter to continue...".to_string()
        };
        
        // Format wait message with special formatting
        let formatted_message = format!(
            "{}⏸️ WORKFLOW PAUSED:{} {}\nPress Enter to continue...",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            message
        );
        
        // Display message and wait for input
        println!("{}", formatted_message);
        
        // Wait for user input
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;
        
        // Let the user know we're continuing
        println!("▶️ Workflow continuing...");
        
        Ok(())
    }
}