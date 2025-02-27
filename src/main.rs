use std::io::{self, Write};
use std::env;
use std::path::Path;
use std::process::Command;
use std::fs;
mod constants;
use serde::{Deserialize, Serialize};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Serialize)]
struct MessageRequest {
    model: String,
    max_tokens: usize,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Serialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct MessageResponse {
    id: String,
    content: Vec<Content>,
    model: String,
}

#[derive(Deserialize, Debug)]
struct Content {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

struct ToolResult {
    success: bool,
    user_output: String,  // Output to show to the user (possibly truncated)
    agent_output: String, // Full output for the agent
}

struct ClaudeClient {
    api_key: String,
    model: String,
    conversation: Vec<Message>,
    system_prompt: Option<String>,
    tools_enabled: bool,
}

impl ClaudeClient {
    fn new(api_key: String, model: String) -> Self {
        use constants::*;
        
        // Create system prompt using template
        let default_system_prompt = SYSTEM_PROMPT_TEMPLATE
            .replace("{TOOL_START}", TOOL_START)
            .replace("{TOOL_END}", TOOL_END)
            .replace("{PATCH_DELIMITER_BEFORE}", PATCH_DELIMITER_BEFORE)
            .replace("{PATCH_DELIMITER_AFTER}", PATCH_DELIMITER_AFTER)
            .replace("{PATCH_DELIMITER_END}", PATCH_DELIMITER_END);

        ClaudeClient {
            api_key,
            model,
            conversation: Vec::new(),
            system_prompt: Some(default_system_prompt),
            tools_enabled: false,
        }
    }

    fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    fn enable_tools(&mut self, enabled: bool) {
        self.tools_enabled = enabled;
    }

    fn send_message(&mut self, user_message: &str) -> Result<String, Box<dyn std::error::Error>> {
        use constants::{TOOL_START, TOOL_END, TOOL_RESULT_START, TOOL_RESULT_END, TOOL_ERROR_START, TOOL_ERROR_END};
        
        // Process the message for tools if tools are enabled
        let processed_message = if self.tools_enabled {
            self.process_tools(user_message)?
        } else {
            user_message.to_string()
        };

        // Add user message to conversation history
        self.conversation.push(Message {
            role: "user".to_string(),
            content: processed_message,
        });

        // Prepare the request
        let request = MessageRequest {
            model: self.model.clone(),
            max_tokens: 4000,
            messages: self.conversation.clone(),
            system: self.system_prompt.clone(),
        };

        // Send the request to Claude API
        let response: MessageResponse = ureq::post(API_URL)
            .set("Content-Type", "application/json")
            .set("X-Api-Key", &self.api_key)
            .set("anthropic-version", ANTHROPIC_VERSION)
            .send_json(serde_json::to_value(request)?)?.into_json()?;

        // Get the assistant's response
        let assistant_message = response.content.iter()
            .filter(|content| content.content_type == "text")
            .map(|content| content.text.clone())
            .collect::<Vec<String>>()
            .join("");

        // This will be the final response to return
        let final_response;
        
        // Special handling for responses with tool invocations
        if self.tools_enabled && assistant_message.contains(TOOL_START) {
            // Find the complete tool invocation (from start to end tag)
            if let Some(tool_start_idx) = assistant_message.find(TOOL_START) {
                if let Some(tool_end_relative_idx) = assistant_message[tool_start_idx..].find(TOOL_END) {
                    // Complete end position (including the end tag)
                    let tool_end_idx = tool_start_idx + tool_end_relative_idx;
                    let complete_end_idx = tool_end_idx + TOOL_END.len();
                    
                    // Everything before and including the tool invocation
                    let assistant_part = assistant_message[0..complete_end_idx].to_string();
                    
                    // Process the tool to get the result
                    let tool_content = &assistant_message[tool_start_idx + TOOL_START.len()..tool_end_idx];
                    
                    // Check if this is the "done" tool
                    let parts: Vec<&str> = tool_content.trim().splitn(2, ' ').collect();
                    let is_done_tool = !parts.is_empty() && parts[0].to_lowercase() == "done";
                    
                    let tool_result = self.execute_tool(tool_content);
                    
                    // Format the tool result as a user message
                    // User sees the user_output, but Claude (agent) gets the agent_output in the conversation history
                    let user_response = if tool_result.success {
                        format!("{}\n{}\n{}", TOOL_RESULT_START, tool_result.user_output, TOOL_RESULT_END)
                    } else {
                        format!("{}\n{}\n{}", TOOL_ERROR_START, tool_result.user_output, TOOL_ERROR_END)
                    };
                    
                    let agent_response = if tool_result.success {
                        format!("{}\n{}\n{}", TOOL_RESULT_START, tool_result.agent_output, TOOL_RESULT_END)
                    } else {
                        format!("{}\n{}\n{}", TOOL_ERROR_START, tool_result.agent_output, TOOL_ERROR_END)
                    };
                    
                    // Add the assistant's response (with tool invocation) to conversation history
                    self.conversation.push(Message {
                        role: "assistant".to_string(),
                        content: assistant_part.clone(),
                    });
                    
                    // Add the agent_response to the conversation history (for Claude to see)
                    self.conversation.push(Message {
                        role: "user".to_string(),
                        content: agent_response,
                    });
                    
                    // For display purposes, show the user the user_response
                    final_response = format!("{}\n{}", assistant_part, user_response);
                    
                    // If this was the "done" tool, we'll return the final response with a special flag
                    if is_done_tool {
                        return Ok(final_response);
                    }
                } else {
                    // Tool start tag found but no end tag - throw an error
                    return Err("Incomplete tool invocation: Found tool start tag but no matching end tag".into());
                }
            } else {
                // No tool invocation found
                self.conversation.push(Message {
                    role: "assistant".to_string(),
                    content: assistant_message.clone(),
                });
                final_response = assistant_message.clone();
            }
        } else {
            return Err("No tool invocation found in response".into());
        }

        Ok(final_response)
    }

    fn clear_conversation(&mut self) {
        self.conversation.clear();
    }

    fn process_tools(&self, message: &str) -> Result<String, Box<dyn std::error::Error>> {
        use constants::{TOOL_START, TOOL_END, TOOL_RESULT_START, TOOL_RESULT_END, TOOL_ERROR_START, TOOL_ERROR_END};
        
        let mut result = message.to_string();
        let mut start_index = 0;

        while let Some(tool_start) = result[start_index..].find(TOOL_START) {
            let tool_start = start_index + tool_start;
            if let Some(tool_end) = result[tool_start..].find(TOOL_END) {
                let tool_end = tool_start + tool_end;
                let tool_content = &result[tool_start + TOOL_START.len()..tool_end];
                
                // Check if this is the "done" tool
                let parts: Vec<&str> = tool_content.trim().splitn(2, ' ').collect();
                let is_done_tool = !parts.is_empty() && parts[0].to_lowercase() == "done";
                
                // Process the tool
                let tool_result = self.execute_tool(tool_content);
                
                // Get the parts of the string we need
                let before_part = result[0..tool_end + TOOL_END.len()].to_string();
                let after_part = result[tool_end + TOOL_END.len()..].to_string();
                
                // Format user output for display
                let user_response = if tool_result.success {
                    format!("\n{}\n{}\n{}\n", TOOL_RESULT_START, tool_result.user_output, TOOL_RESULT_END)
                } else {
                    format!("\n{}\n{}\n{}\n", TOOL_ERROR_START, tool_result.user_output, TOOL_ERROR_END)
                };
                
                // Format agent output for conversation history
                // We don't use this variable directly here, but we're documenting the structure
                // The actual agent output is processed separately in send_message
                let _agent_response = if tool_result.success {
                    format!("\n{}\n{}\n{}\n", TOOL_RESULT_START, tool_result.agent_output, TOOL_RESULT_END)
                } else {
                    format!("\n{}\n{}\n{}\n", TOOL_ERROR_START, tool_result.agent_output, TOOL_ERROR_END)
                };
                
                // Note: agent_response is intentionally not used here since we're
                // only constructing the visible output. The agent_output is handled separately
                // when added to the conversation history via send_message.
                
                // For the visible output to the user, we use user_response
                result = format!("{}{}{}", before_part, user_response, after_part);
                
                // Update the start index to continue searching after the response
                start_index = before_part.len() + user_response.len();
                
                // Special handling for the "done" tool - we'll keep processing other tools that might be in the message
                if is_done_tool && after_part.trim().is_empty() {
                    break;
                }
            } else {
                // No closing tag found, return an error
                return Err(format!("Incomplete tool invocation: Found tool start tag at position {} but no matching end tag", tool_start).into());
            }
        }
        
        Ok(result)
    }

    fn execute_tool(&self, tool_content: &str) -> ToolResult {
        let parts: Vec<&str> = tool_content.trim().splitn(2, ' ').collect();
        if parts.is_empty() {
            let error_msg = "Empty tool invocation".to_string();
            return ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            };
        }

        let tool_name = parts[0];
        let args = if parts.len() > 1 { parts[1] } else { "" };

        match tool_name.to_lowercase().as_str() {
            "shell" => self.execute_shell(args),
            "read" => self.execute_read(args),
            "write" => self.execute_write(args),
            "patch" => self.execute_patch(args),
            "done" => self.execute_done(args),
            _ => {
                let error_msg = format!("Unknown tool: {}", tool_name);
                ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                }
            },
        }
    }

    fn execute_shell(&self, args: &str) -> ToolResult {
        let shell = if cfg!(target_os = "windows") {
            "cmd"
        } else {
            "sh"
        };

        let shell_arg = if cfg!(target_os = "windows") {
            "/C"
        } else {
            "-c"
        };

        match Command::new(shell)
            .arg(shell_arg)
            .arg(args)
            .output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    
                    if output.status.success() {
                        // Full output for the agent
                        let agent_output = format!("Command: {}\nOutput:\n\n{}", args, stdout);
                        
                        // Limit output for user display
                        let user_output = if stdout.lines().count() > 50 {
                            let first_lines: Vec<&str> = stdout.lines().take(20).collect();
                            let last_lines: Vec<&str> = stdout.lines().rev().take(20).collect();
                            
                            format!(
                                "Command: {}\nOutput (truncated, showing first 20 and last 20 lines of {} total):\n\n{}\n\n[...]\n\n{}", 
                                args,
                                stdout.lines().count(),
                                first_lines.join("\n"),
                                last_lines.into_iter().rev().collect::<Vec<&str>>().join("\n")
                            )
                        } else {
                            agent_output.clone() // For smaller outputs, show the same to user and agent
                        };
                        
                        ToolResult {
                            success: true,
                            user_output,
                            agent_output,
                        }
                    } else {
                        let error_msg = format!("Error executing command '{}': {}", args, stderr);
                        ToolResult {
                            success: false,
                            user_output: error_msg.clone(),
                            agent_output: error_msg,
                        }
                    }
                },
                Err(e) => {
                    let error_msg = format!("Failed to execute command '{}': {}", args, e);
                    ToolResult {
                        success: false,
                        user_output: error_msg.clone(),
                        agent_output: error_msg,
                    }
                },
            }
    }

    fn execute_read(&self, args: &str) -> ToolResult {
        // Parse arguments with optional offset and limit
        let mut offset: Option<usize> = None;
        let mut limit: Option<usize> = None;
        let mut filepath_str = args.trim().to_string();
        
        // Look for offset parameter
        if let Some(offset_idx) = args.find("offset=") {
            let offset_end = match args[offset_idx..].find(' ') {
                Some(end) => offset_idx + end,
                None => args.len(),
            };
            let offset_str = &args[offset_idx + 7..offset_end];
            if let Ok(val) = offset_str.parse::<usize>() {
                offset = Some(val);
            }
            let replacement = String::from(&args[offset_idx..offset_end]);
            filepath_str = filepath_str.replace(&replacement, "");
        }
        
        // Look for limit parameter
        if let Some(limit_idx) = args.find("limit=") {
            let limit_end = match args[limit_idx..].find(' ') {
                Some(end) => limit_idx + end,
                None => args.len(),
            };
            let limit_str = &args[limit_idx + 6..limit_end];
            if let Ok(val) = limit_str.parse::<usize>() {
                limit = Some(val);
            }
            let replacement = String::from(&args[limit_idx..limit_end]);
            filepath_str = filepath_str.replace(&replacement, "");
        }
        
        // Trim any extra whitespace
        let filepath = filepath_str.trim();
        
        // Read the file
        match fs::read_to_string(filepath) {
            Ok(content) => {
                // Split content into lines
                let lines: Vec<&str> = content.lines().collect();
                let total_lines = lines.len();
                
                // Apply offset and limit for agent output
                let agent_start_line = offset.unwrap_or(0).min(total_lines);
                let agent_end_line = match limit {
                    Some(l) => (agent_start_line + l).min(total_lines),
                    None => total_lines,
                };
                
                // Full content for the agent (respecting offset/limit if provided)
                let agent_lines = lines[agent_start_line..agent_end_line].join("\n");
                let agent_output = format!(
                    "File: {} (lines {}-{} of {})\n\n```\n{}\n```",
                    filepath, agent_start_line+1, agent_end_line, total_lines, agent_lines
                );
                
                // Truncated content for the user
                let user_output = if total_lines > 100 {
                    // For large files, show first 20 and last 20 lines
                    let first_20 = lines.iter().take(20).cloned().collect::<Vec<&str>>().join("\n");
                    let last_20 = lines.iter().rev().take(20).rev().cloned().collect::<Vec<&str>>().join("\n");
                    
                    format!(
                        "File: {} (showing first/last 20 lines of {} total)\n\n```\n{}\n\n[...{} lines omitted...]\n\n{}\n```",
                        filepath, total_lines, first_20, total_lines - 40, last_20
                    )
                } else {
                    // For smaller files, show the same content as the agent
                    agent_output.clone()
                };
                
                ToolResult {
                    success: true,
                    user_output,
                    agent_output,
                }
            },
            Err(e) => {
                let error_msg = format!("Error reading file '{}': {}", filepath, e);
                ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                }
            },
        }
    }

    fn execute_write(&self, args: &str) -> ToolResult {
        let lines: Vec<&str> = args.trim().lines().collect();
        if lines.is_empty() {
            let error_msg = "Write tool requires a filename followed by content on new lines".to_string();
            return ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            };
        }

        let filename = lines[0].trim();
        let content = if lines.len() > 1 {
            lines[1..].join("\n")
        } else {
            "".to_string()
        };

        match fs::write(filename, content) {
            Ok(_) => {
                let msg = format!("Successfully wrote to file '{}'", filename);
                ToolResult {
                    success: true,
                    user_output: msg.clone(),
                    agent_output: msg,
                }
            },
            Err(e) => {
                let error_msg = format!("Error writing to file '{}': {}", filename, e);
                ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                }
            },
        }
    }
    
    fn execute_patch(&self, args: &str) -> ToolResult {
        use constants::{PATCH_DELIMITER_BEFORE, PATCH_DELIMITER_AFTER, PATCH_DELIMITER_END};
        
        let lines: Vec<&str> = args.trim().lines().collect();
        if lines.is_empty() {
            let error_msg = "Patch tool requires a filename and patch content".to_string();
            return ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            };
        }

        let filename = lines[0].trim();
        let patch_content = lines[1..].join("\n");
        
        // Read the file content
        let file_content = match fs::read_to_string(filename) {
            Ok(content) => content,
            Err(e) => {
                let error_msg = format!("Error reading file '{}': {}", filename, e);
                return ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                };
            }
        };
        
        // Parse the patch content
        let before_delimiter = match patch_content.find(PATCH_DELIMITER_BEFORE) {
            Some(pos) => pos,
            None => {
                let error_msg = format!("Missing '{}' delimiter in patch", PATCH_DELIMITER_BEFORE);
                return ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                };
            }
        };
        
        let after_delimiter = match patch_content[before_delimiter..].find(PATCH_DELIMITER_AFTER) {
            Some(pos) => before_delimiter + pos,
            None => {
                let error_msg = format!("Missing '{}' delimiter in patch", PATCH_DELIMITER_AFTER);
                return ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                };
            }
        };
        
        let end_delimiter = match patch_content[after_delimiter..].find(PATCH_DELIMITER_END) {
            Some(pos) => after_delimiter + pos,
            None => {
                let error_msg = format!("Missing '{}' delimiter in patch", PATCH_DELIMITER_END);
                return ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                };
            }
        };
        
        // Extract the before and after text
        // Skip the delimiter line itself by finding the next newline
        let before_start = match patch_content[before_delimiter + PATCH_DELIMITER_BEFORE.len()..].find('\n') {
            Some(pos) => before_delimiter + PATCH_DELIMITER_BEFORE.len() + pos + 1,
            None => before_delimiter + PATCH_DELIMITER_BEFORE.len(),
        };
        
        let after_start = match patch_content[after_delimiter + PATCH_DELIMITER_AFTER.len()..].find('\n') {
            Some(pos) => after_delimiter + PATCH_DELIMITER_AFTER.len() + pos + 1,
            None => after_delimiter + PATCH_DELIMITER_AFTER.len(),
        };
        
        let before_text = patch_content[before_start..after_delimiter].trim();
        let after_text = patch_content[after_start..end_delimiter].trim();
        
        // Apply the patch
        if !file_content.contains(before_text) {
            let error_msg = format!("Text to replace not found in the file: '{}'", before_text);
            return ToolResult {
                success: false,
                user_output: error_msg.clone(),
                agent_output: error_msg,
            };
        }
        
        let new_content = file_content.replace(before_text, after_text);
        
        // Write the updated content
        match fs::write(filename, new_content) {
            Ok(_) => {
                // Full output for the agent
                let agent_output = format!("Successfully patched file '{}'", filename);
                
                // Create a pretty diff for the user output
                let before_lines = before_text.lines();
                let after_lines = after_text.lines();
                
                // Format with colored backgrounds (ANSI escape codes)
                let red_bg = "\u{1b}[41;97m";    // Light red background with white text
                let green_bg = "\u{1b}[42;97m";  // Light green background with white text
                let reset = "\u{1b}[0m";         // Reset formatting
                
                let removed = before_lines
                    .map(|line| format!("{}- {}{}", red_bg, line, reset))
                    .collect::<Vec<String>>()
                    .join("\n");
                
                let added = after_lines
                    .map(|line| format!("{}+ {}{}", green_bg, line, reset))
                    .collect::<Vec<String>>()
                    .join("\n");
                
                let user_output = format!(
                    "Successfully patched file '{}'\n\nDiff:\n{}\n{}",
                    filename, removed, added
                );
                
                ToolResult {
                    success: true,
                    user_output,
                    agent_output,
                }
            },
            Err(e) => {
                let error_msg = format!("Error writing patched file '{}': {}", filename, e);
                ToolResult {
                    success: false,
                    user_output: error_msg.clone(),
                    agent_output: error_msg,
                }
            },
        }
    }
    
    fn execute_done(&self, args: &str) -> ToolResult {
        let summary = if args.trim().is_empty() {
            "Task completed successfully."
        } else {
            args.trim()
        };
        
        let output = format!("TASK COMPLETE:\n{}\n\nNo further agent actions will be taken.", summary);
        ToolResult {
            success: true,
            user_output: output.clone(),
            agent_output: output,
        }
    }
}

fn display_help() {
    use constants::*;
    
    // Create help text using template
    let help_text = HELP_TEMPLATE
        .replace("{TOOL_START}", TOOL_START)
        .replace("{TOOL_END}", TOOL_END)
        .replace("{PATCH_DELIMITER_BEFORE}", PATCH_DELIMITER_BEFORE)
        .replace("{PATCH_DELIMITER_AFTER}", PATCH_DELIMITER_AFTER)
        .replace("{PATCH_DELIMITER_END}", PATCH_DELIMITER_END);
    
    println!("{}", help_text);
}

fn read_line() -> Result<String, Box<dyn std::error::Error>> {
    // Simply use standard readline for simplicity
    let mut input = String::new();
    
    // Print the prompt
    print!("> ");
    io::stdout().flush()?;
    
    // Read input
    io::stdin().read_line(&mut input)?;
    
    // Trim newline
    input = input.trim_end().to_string();
    
    Ok(input)
}

fn process_user_query(client: &mut ClaudeClient, user_input: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let mut message_sent = false;
    let mut task_completed = false;

    println!("\nClaude is working on your request...");
    
    // Loop until we get a "done" tool or encounter an error
    loop {
        // Send the message to Claude
        let response = if !message_sent {
            // First message is the user's input
            message_sent = true;
            client.send_message(user_input)
        } else {
            // Subsequent messages are empty - Claude will continue with tool output
            client.send_message("")
        };
        
        match response {
            Ok(response) => {
                println!("\nClaude:");
                println!("{}", response);
                println!("");
                
                // Check if the response contains the "done" tool result
                if response.contains("TASK COMPLETE:") {
                    println!("\nTask completed. Agent has signaled completion using the 'done' tool.");
                    task_completed = true;
                    break;
                }
                
                // Small delay to prevent tight loop
                std::thread::sleep(std::time::Duration::from_millis(100));
            },
            Err(err) => {
                // Only show error if it's not the expected pattern for end of conversation
                if !err.to_string().contains("No tool invocation found in response") {
                    println!("\nError: {}\n", err);
                }
                break;
            }
        }

    }
    
    Ok(task_completed)
}

fn run_interactive_mode(api_key: String, model: String, enable_tools: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ClaudeClient::new(api_key.clone(), model);
    client.enable_tools(enable_tools);
    
    println!("Claude Console Interface");
    println!("Type your message and press Enter to chat with Claude");
    println!("Type '/help' for available commands or '/exit' to quit");
    
    if enable_tools {
        println!("Tools are ENABLED. Claude will use tools automatically based on your request.");
    } else {
        println!("Tools are DISABLED. Use /tools on to enable them.");
    }
    println!();
    
    loop {
        let user_input = match read_line() {
            Ok(input) => input,
            Err(e) => {
                println!("Input error: {}", e);
                continue;
            }
        };
        
        let user_input = user_input.trim();
        
        // Handle commands
        if user_input.starts_with("/") {
            let parts: Vec<&str> = user_input.splitn(2, ' ').collect();
            let command = parts[0].to_lowercase();
            
            match command.as_str() {
                "/exit" => break,
                
                "/help" => display_help(),
                
                "/clear" => {
                    client.clear_conversation();
                    println!("Conversation history cleared.");
                },
                
                "/system" => {
                    if parts.len() > 1 {
                        let system_prompt = parts[1].to_string();
                        client.set_system_prompt(system_prompt);
                        println!("System prompt set.");
                    } else {
                        println!("Usage: /system YOUR SYSTEM PROMPT TEXT");
                    }
                },
                
                "/model" => {
                    if parts.len() > 1 {
                        let model_name = parts[1].to_string();
                        let tools_enabled = client.tools_enabled;
                        client = ClaudeClient::new(api_key.clone(), model_name);
                        client.enable_tools(tools_enabled);
                        println!("Model changed. Conversation history cleared.");
                    } else {
                        println!("Usage: /model MODEL_NAME");
                        println!("Examples: claude-3-opus-20240229, claude-3-sonnet-20240229, claude-3-haiku-20240307");
                    }
                },
                
                "/tools" => {
                    if parts.len() > 1 {
                        match parts[1].to_lowercase().as_str() {
                            "on" | "enable" | "true" => {
                                client.enable_tools(true);
                                println!("Tools enabled. Claude will use tools automatically based on your request.");
                            },
                            "off" | "disable" | "false" => {
                                client.enable_tools(false);
                                println!("Tools disabled.");
                            },
                            _ => println!("Usage: /tools on|off"),
                        }
                    } else {
                        println!("Usage: /tools on|off");
                    }
                },
                
                _ => println!("Unknown command. Type /help for available commands."),
            }
            
            continue;
        }
        
        // Process user query
        if !user_input.is_empty() {
            let _ = process_user_query(&mut client, user_input)?;
        }
    }
    
    println!("Goodbye!");
    Ok(())
}

fn run_query(api_key: String, model: String, query: &str, system_prompt: Option<String>, enable_tools: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ClaudeClient::new(api_key, model);
    
    if let Some(prompt) = system_prompt {
        client.set_system_prompt(prompt);
    }
    
    client.enable_tools(enable_tools);
    
    // Process the query
    let result = process_user_query(&mut client, query);
    
    result.map(|_| ())
}

fn print_usage() {
    use constants::*;
    
    // Create usage text using template
    let usage_text = USAGE_TEMPLATE
        .replace("{TOOL_START}", TOOL_START)
        .replace("{TOOL_END}", TOOL_END)
        .replace("{PATCH_DELIMITER_BEFORE}", PATCH_DELIMITER_BEFORE)
        .replace("{PATCH_DELIMITER_AFTER}", PATCH_DELIMITER_AFTER)
        .replace("{PATCH_DELIMITER_END}", PATCH_DELIMITER_END);
    
    eprintln!("{}", usage_text);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if exists
    let _ = dotenvy::dotenv();
    
    // Try custom .env locations if default not found
    if env::var("ANTHROPIC_API_KEY").is_err() {
        for env_path in ["./env/.env", "../.env", "~/.env"] {
            if Path::new(env_path).exists() {
                let _ = dotenvy::from_path(env_path);
                break;
            }
        }
    }
    
    // Get API key from environment variable
    let api_key = match env::var("ANTHROPIC_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("Error: ANTHROPIC_API_KEY environment variable not set");
            eprintln!("Please set it in a .env file or as an environment variable");
            eprintln!("Example .env file content:");
            eprintln!("ANTHROPIC_API_KEY=your_api_key_here");
            return Ok(());
        }
    };
    
    // Initialize defaults
    let default_model = "claude-3-7-sonnet-20250219".to_string();
    
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    let mut model = default_model;
    let mut system_prompt = None;
    let mut query = None;
    let mut show_help = false;
    let mut enable_tools = true; // Enable tools by default
    
    while i < args.len() {
        match args[i].as_str() {
            "--model" => {
                if i + 1 < args.len() {
                    model = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --model requires a MODEL_NAME");
                    return Ok(());
                }
            },
            "--system" => {
                if i + 1 < args.len() {
                    system_prompt = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("Error: --system requires a PROMPT");
                    return Ok(());
                }
            },
            "--no-tools" => {
                enable_tools = false;
                i += 1;
            },
            "--help" => {
                show_help = true;
                i += 1;
            },
            _ => {
                // If it doesn't start with --, treat it as the query
                if !args[i].starts_with("--") && query.is_none() {
                    query = Some(args[i].clone());
                }
                i += 1;
            }
        }
    }
    
    if show_help {
        print_usage();
        return Ok(());
    }
    
    // Determine mode and run
    match query {
        Some(q) => run_query(api_key, model, &q, system_prompt, enable_tools)?,
        None => run_interactive_mode(api_key, model, enable_tools)?,
    }
    
    Ok(())
}