use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::ClaudeClient;
use crate::{process_user_query, Content};

pub fn execute_task(args: &str, body: &str) -> ToolResult {
    // Use args as the task name/description and body as task instructions
    let task_name = args.trim();
    let task_instructions = body.trim();
    
    if task_instructions.is_empty() {
        return ToolResult {
            success: false,
            user_output: "Error: Task requires instructions in the body".to_string(),
            agent_output: "Error: Task requires instructions in the body".to_string(),
        };
    }
    
    // Create user output information with nice formatting using emoji and consistent style
    let start_message = format!("{}🔄 Subagent Task Started:{} {}", 
                             FORMAT_BOLD, FORMAT_RESET, 
                             task_name);
    
    // Create preview of task instructions in gray
    let instructions_preview = format!("\n{}{}{}", FORMAT_GRAY, task_instructions, FORMAT_RESET);
    
    // Display the start message immediately so user knows a subtask is starting
    println!("\n{}{}\n", start_message, instructions_preview);
    
    // Try to create a subagent and execute the task
    match execute_subagent_task(task_name, task_instructions) {
        Ok(result) => {
            // Create a completion message with the same nice formatting using emoji
            let completion_message = format!("{}✅ Subagent Task Completed:{} {}", 
                                         FORMAT_BOLD, FORMAT_RESET, 
                                         task_name);
            
            // Format the result as a gray preview
            let result_preview = format!("\n{}{}{}", FORMAT_GRAY, result, FORMAT_RESET);
            
            // Print the completion message for immediate feedback
            println!("\n{}{}\n", completion_message, result_preview);
            
            // Include both messages in the user_output for history
            let full_output = format!("{}{}\n\n{}{}", 
                                    start_message, 
                                    instructions_preview,
                                    completion_message,
                                    result_preview);
            
            ToolResult {
                success: true,
                user_output: full_output,
                // Pass the subagent result directly to the main agent without any wrapper
                agent_output: result,
            }
        },
        Err(err) => {
            // Format the error message with nice styling using emoji
            let error_message = format!("{}❌ Subagent Task Error:{} {}", 
                                     FORMAT_BOLD, FORMAT_RESET, 
                                     task_name);
            
            // Format the error details as a gray preview
            let error_preview = format!("\n{}{}{}", FORMAT_GRAY, err, FORMAT_RESET);
            
            // Print the error message for immediate feedback
            println!("\n{}{}\n", error_message, error_preview);
            
            ToolResult {
                success: false,
                user_output: format!("{}{}\n\n{}{}", 
                                   start_message, 
                                   instructions_preview,
                                   error_message, 
                                   error_preview),
                agent_output: format!("Error executing subagent task: {}", err),
            }
        }
    }
}

// Create a subagent and execute the given task
fn execute_subagent_task(task_name: &str, instructions: &str) -> Result<String, String> {
    // Create a new subagent configured for this task
    let mut subagent = match ClaudeClient::create_subagent_for_task(task_name) {
        Ok(agent) => agent,
        Err(e) => return Err(format!("Failed to create subagent: {}", e)),
    };
    
    // Send the task instructions to the subagent and process until completion
    let task_result = match process_user_query(&mut subagent, instructions, false) {
        Ok((_, _result)) => {
            // First try to find a message with the done tool
            if let Some(last_message) = subagent.conversation.iter().rev().find(|m| {
                matches!(m.info, 
                    crate::MessageInfo::ToolResult { ref tool_name } | 
                    crate::MessageInfo::ToolError { ref tool_name } 
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