use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::agent::{Agent, process_user_query};
use crate::llm::{Content, MessageInfo};

pub async fn execute_task(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // Parse the args string to check for model parameter
    let mut task_name = args.trim().to_string();
    let mut model_name = None;
    
    // Check for --model parameter in the args
    if let Some(model_pos) = task_name.find("--model") {
        // Extract the part of the string from the --model flag
        let model_part = &task_name[model_pos..];
        
        // Parse the model name - it should be after "--model " 
        if let Some(space_pos) = model_part.find(' ') {
            // Check that there is a model name after the space
            if space_pos + 1 < model_part.len() {
                // Extract the model name up to the next space or end of string
                let remaining = &model_part[space_pos + 1..];
                let end_pos = remaining.find(' ').unwrap_or(remaining.len());
                model_name = Some(remaining[..end_pos].to_string());
                
                // Remove the model parameter from the task name
                // This recreates the task name without the --model parameter
                let mut clean_task_name = task_name[..model_pos].trim().to_string();
                if model_pos + space_pos + 1 + end_pos < task_name.len() {
                    if !clean_task_name.is_empty() {
                        clean_task_name.push(' ');
                    }
                    clean_task_name.push_str(task_name[model_pos + space_pos + 1 + end_pos..].trim());
                }
                task_name = clean_task_name;
            }
        }
    }
    
    let task_instructions = body.trim();
    
    if task_instructions.is_empty() {
        let error_msg = "Error: Task requires instructions in the body".to_string();
        
        if !silent_mode {
            println!("{}❌ Error:{} {}", 
                FORMAT_BOLD, FORMAT_RESET, error_msg);
        }
        
        return ToolResult {
            success: false,
            agent_output: error_msg,
        };
    }
    
    // Print model information if specified and not in silent mode
    if !silent_mode {
        // Create start message with nice formatting using emoji and consistent style
        let start_message = format!("{}🔄 Subagent Task Started:{} {}", 
                                FORMAT_BOLD, FORMAT_RESET, 
                                task_name);
        
        // Add model information if provided
        let model_info = if let Some(model) = &model_name {
            format!("\n{}Using model: {}{}", FORMAT_GRAY, model, FORMAT_RESET)
        } else {
            String::new()
        };
        
        // Create preview of task instructions in gray
        let instructions_preview = format!("\n{}{}{}", FORMAT_GRAY, task_instructions, FORMAT_RESET);
        
        // Display the start message immediately so user knows a subtask is starting
        println!("\n{}{}{}\n", start_message, model_info, instructions_preview);
    }
    // Pin here because recursion
    let pinned_task = Box::pin(execute_subagent_task(&task_name, task_instructions, model_name, silent_mode));
    // Try to create a subagent and execute the task
    match pinned_task.await {
        Ok(result) => {
            // Only print completion if not in silent mode
            if !silent_mode {
                // Create a completion message with the same nice formatting using emoji
                let completion_message = format!("{}✅ Subagent Task Completed:{} {}", 
                                            FORMAT_BOLD, FORMAT_RESET, 
                                            task_name);
                
                // Format the result as a gray preview
                let result_preview = format!("\n{}{}{}", FORMAT_GRAY, result, FORMAT_RESET);
                
                // Print the completion message for immediate feedback
                println!("\n{}{}\n", completion_message, result_preview);
            }
            
            ToolResult {
                success: true,
                // Pass the subagent result directly to the main agent without any wrapper
                agent_output: result,
            }
        },
        Err(err) => {
            // Only print error if not in silent mode
            if !silent_mode {
                // Format the error message with nice styling using emoji
                let error_message = format!("{}❌ Subagent Task Error:{} {}", 
                                         FORMAT_BOLD, FORMAT_RESET, 
                                         task_name);
                
                // Format the error details as a gray preview
                let error_preview = format!("\n{}{}{}", FORMAT_GRAY, err, FORMAT_RESET);
                
                // Print the error message for immediate feedback
                println!("\n{}{}\n", error_message, error_preview);
            }
            
            ToolResult {
                success: false,
                agent_output: format!("Error executing subagent task: {}", err),
            }
        }
    }
}

// Create a subagent and execute the given task
async fn execute_subagent_task(task_name: &str, instructions: &str, model: Option<String>, silent_mode: bool) -> Result<String, String> {
    // Create a new subagent configured for this task
    // This will use the default provider (Anthropic)
    let mut subagent = match Agent::create_subagent_for_task(task_name, model) {
        Ok(agent) => agent,
        Err(e) => return Err(format!("Failed to create subagent: {}", e)),
    };
    
    // Send the task instructions to the subagent and process until completion
    // Use silent_mode parameter to control subagent output
    let task_result = match process_user_query(&mut subagent, instructions, silent_mode).await {
        Ok((_, _result)) => {
            // First try to find a message with the done tool
            if let Some(last_message) = subagent.conversation.iter().rev().find(|m| {
                matches!(m.info, 
                    MessageInfo::ToolResult { ref tool_name } | 
                    MessageInfo::ToolError { ref tool_name } 
                    if tool_name == "done"
                )
            }) {
                // Extract the done tool's result as text
                match &last_message.content {
                    Content::Text { text } => text.clone(),
                    _ => "Done message found but content was not in text format".to_string(),
                }
            } else {
                // If no done tool was found, return all responses concatenated
                let mut responses = Vec::new();
                
                for message in subagent.conversation.iter() {
                    if message.role == "assistant" {
                        if let Content::Text { text } = &message.content {
                            responses.push(text.clone());
                        } else {
                            responses.push("[Non-text content]".to_string());
                        }
                    }
                }
                
                if responses.is_empty() {
                    "No responses from subagent".to_string()
                } else {
                    responses.join("\n\n--- Next Response ---\n\n")
                }
            }
        },
        Err(e) => return Err(format!("Subagent task error: {}", e)),
    };
    
    Ok(task_result)
}