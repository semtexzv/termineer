//! Executor for workflow steps
//!
//! Handles executing each step in a workflow, managing the context,
//! and coordinating between steps.

use std::collections::HashMap;
use std::process::Command;
use std::io;

use crate::agent::{AgentId, AgentMessage};
use crate::workflow::types::{Workflow, Step, StepType};
use crate::workflow::context::{WorkflowContext, WorkflowError};

/// Executor for workflows
pub struct WorkflowExecutor {
    // The struct is kept for API compatibility, but doesn't need to store any fields
}

impl WorkflowExecutor {
    /// Create a new workflow executor
    pub fn new(_agent_id: crate::agent::AgentId) -> Self {
        Self { }
    }
    
    /// Execute a workflow with the given parameters and optional query
    pub async fn execute_workflow(
        &self,
        workflow: &Workflow,
        parameters: HashMap<String, serde_yaml::Value>,
        query: Option<String>,
    ) -> Result<(), WorkflowError> {
        // Check if user has Pro access - workflows are a Pro-only feature
        if crate::config::get_app_mode() != crate::config::AppMode::Pro {
            return Err(WorkflowError::PermissionDenied(
                "Workflows are a Pro-only feature. Upgrade to Pro for access.".to_string()
            ));
        }
        
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
            
            // Enhanced logging with more visible separation between steps
            println!("\n{}", "=".repeat(80));
            println!("📋 STEP {}/{}: {} - {}", 
                     step_index + 1, 
                     workflow.steps.len(),
                     step_id, 
                     step.description.as_deref().unwrap_or(""));
            println!("{}\n", "-".repeat(80));
            
            // Logging to monitor execution
            println!("Executing step with type: {}", step_type);
            
            match step_type {
                StepType::Shell => {
                    self.execute_shell_step(step, &mut context).await?;
                },
                StepType::Agent => {
                    println!("Executing agent step: {}", step.get_id());
                    self.execute_agent_step(step, &mut context).await?;
                },
                StepType::Unknown => {
                    return Err(WorkflowError::InvalidStepType);
                }
            }
        }
        
        println!("\n{}", "=".repeat(80));
        println!("✅ WORKFLOW COMPLETED SUCCESSFULLY: {}", workflow.name);
        println!("{}", "=".repeat(80));
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
        println!("🔄 Executing shell command: {}", rendered_command);
        println!("{}", "-".repeat(40));
        
        let output = self.execute_shell_command(&rendered_command)
            .map_err(|e| WorkflowError::ShellError(e.to_string()))?;
        
        // Store output if specified
        if let Some(var_name) = &step.store_output {
            context.set_variable(var_name.clone(), output.clone());
            println!("✅ Command output stored in variable: {}", var_name);
            
            // Log a preview of the output (truncated if very long)
            let preview = if output.len() > 500 {
                format!("{}... [truncated {} more characters]", 
                        &output[..500], output.len() - 500)
            } else {
                output.clone()
            };
            
            println!("\n📄 Output preview: \n{}\n", preview);
        } else {
            // Always show output preview if not stored in a variable
            println!("\n📄 Command output: \n{}\n", output);
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
    
    /// Execute an agent step with streaming output
    async fn execute_agent_step(
        &self, 
        step: &Step,
        context: &mut WorkflowContext,
    ) -> Result<(), WorkflowError> {
        use std::time::{Duration, Instant};
        use tokio::time::sleep;
        
        // Verify required fields
        let agent_id = step.get_id();
        let prompt_template = step.prompt.as_ref()
            .ok_or(WorkflowError::MissingField("prompt".to_string()))?;
        
        // Render prompt with variable interpolation
        let rendered_prompt = context.render_template(prompt_template)?;
        
        // Log agent step with formatted header
        println!("🤖 Creating agent: {}", agent_id);
        
        // Use specified kind or default to "general"
        let kind = step.kind.as_ref().map(|k| k.as_str()).unwrap_or("general");
        println!("Agent kind: {}", kind);
        println!("{}", "-".repeat(40));
                
        // Log the prompt being sent (truncated if very long)
        let preview = if rendered_prompt.len() > 500 {
            format!("{}... [truncated {} more characters]", 
                    &rendered_prompt[..500], rendered_prompt.len() - 500)
        } else {
            rendered_prompt.clone()
        };
        
        println!("📨 Agent prompt:\n{}\n", preview);
        
        // Create a config for the new agent with the specified kind
        let mut agent_config = crate::config::Config::new();
        agent_config.kind = Some(kind.to_string());
        
        // Create a new agent with the generated name and config
        let agent_name = format!("workflow_agent_{}", agent_id);
        let new_agent_id = crate::agent::create_agent(agent_name, agent_config)
            .map_err(|e| WorkflowError::AgentError(format!("Failed to create agent: {}", e)))?;
        
        // Set up buffer streaming for real-time feedback
        let mut last_line_count = 0;
        let mut buffer_check_time = Instant::now();
        let buffer_check_interval = Duration::from_millis(100);
        let state_check_interval = Duration::from_millis(500);
        let mut state_check_time = Instant::now();
        
        // Send the message to the agent
        crate::agent::send_message(
            new_agent_id, 
            AgentMessage::UserInput(rendered_prompt)
        ).map_err(|e| WorkflowError::AgentError(format!("Failed to send message: {}", e)))?;
        
        println!("Agent is now processing, waiting for completion...");
        println!("{}", "-".repeat(40));
        
        // Use a custom timeout of 5 minutes (300 seconds)
        let timeout_seconds = 300;
        let timeout = Duration::from_secs(timeout_seconds);
        let start_time = Instant::now();
        let mut last_activity_time = Instant::now();
        
        // Use a manual approach that combines buffer streaming and state checking
        let mut response = String::new();
        let mut done = false;
        
        // Keep checking until we're done or reach timeout
        while !done && start_time.elapsed() < timeout {
            let mut had_activity = false;
            
            // Sleep briefly to avoid tight polling
            sleep(Duration::from_millis(50)).await;
            
            // 1. Stream buffer updates only at certain intervals
            if buffer_check_time.elapsed() >= buffer_check_interval {
                buffer_check_time = Instant::now();
                
                if let Ok(buffer) = crate::agent::get_agent_buffer(new_agent_id) {
                    let lines = buffer.lines();
                    let current_count = lines.len();
                    
                    // Check if we have new lines
                    if current_count > last_line_count {
                        had_activity = true;
                        
                        // Print new lines with a subtle prefix
                        for i in last_line_count..current_count {
                            if let Some(line) = lines.get(i) {
                                // Filter out certain system messages for cleaner output
                                if !line.content.starts_with("🤖") && 
                                   !line.content.contains("Token usage:") {
                                    println!("│ {}", line.content);
                                }
                            }
                        }
                        last_line_count = current_count;
                    }
                }
            }
            
            // 2. Check if agent is done (less frequently than buffer checks)
            if state_check_time.elapsed() >= state_check_interval {
                state_check_time = Instant::now();
                
                if let Ok(state) = crate::agent::get_agent_state(new_agent_id) {
                    match state {
                        crate::agent::AgentState::Done(Some(content)) => {
                            // Agent is done with a response
                            response = content;
                            done = true;
                            break;
                        },
                        crate::agent::AgentState::Terminated => {
                            // Agent was terminated
                            return Err(WorkflowError::AgentError("Agent was terminated".to_string()));
                        },
                        crate::agent::AgentState::Processing |
                        crate::agent::AgentState::RunningTool { .. } => {
                            // These are active states, update activity timestamp
                            had_activity = true;
                        },
                        _ => {}
                    }
                }
            }
            
            // Update the last activity time if we saw activity
            if had_activity {
                last_activity_time = Instant::now();
            }
            
            // Additional check: if no activity for 30 seconds, check more aggressively
            if last_activity_time.elapsed() > Duration::from_secs(30) {
                // Force check agent state at next iteration
                state_check_time = Instant::now() - state_check_interval;
            }
        }
        
        // If we reached here and we're not done, we timed out
        if !done {
            // Try to terminate the agent cleanly
            let _ = crate::agent::send_message(new_agent_id, AgentMessage::Terminate);
            
            return Err(WorkflowError::AgentError(
                format!("Agent did not complete within {} seconds", timeout_seconds)
            ));
        }
        
        println!("{}", "-".repeat(40));
        println!("✅ Agent task completed!");
        
        // Store response in the specified variable
        if let Some(var_name) = &step.into {
            context.set_variable(var_name.clone(), response.clone());
            println!("Response stored in variable: {}", var_name);
            
            // Log a preview of the response
            let preview = if response.len() > 500 {
                format!("{}... [truncated {} more characters]", 
                        &response[..500], response.len() - 500)
            } else {
                response.clone()
            };
            
            println!("\n📩 Response preview:\n{}\n", preview);
        } else {
            // Always show a preview if not stored in a variable
            println!("\n📝 Agent response (not stored in any variable):\n{}\n", response);
        }
        
        // Send a terminate message to the agent
        if let Err(e) = crate::agent::send_message(new_agent_id, AgentMessage::Terminate) {
            println!("Warning: Failed to send terminate message to agent: {}", e);
        }
        
        Ok(())
    }
}

/// Execute a workflow with the given parameters and main agent
pub async fn execute_workflow(
    workflow: &Workflow,
    parameters: HashMap<String, serde_yaml::Value>,
    query: Option<String>,
    main_agent_id: AgentId,
) -> Result<(), WorkflowError> {
    // Check if user has Pro access - workflows are a Pro-only feature
    if crate::config::get_app_mode() != crate::config::AppMode::Pro {
        return Err(WorkflowError::PermissionDenied(
            "Workflows are a Pro-only feature. Upgrade to Pro for access.".to_string()
        ));
    }
    
    let executor = WorkflowExecutor::new(main_agent_id);
    executor.execute_workflow(workflow, parameters, query).await
}