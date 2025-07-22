//! Task tool implementation for creating and running subtasks

use crate::agent::{AgentId, AgentMessage, AgentState};
use crate::config::Config;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::prompts;
use crate::tools::ToolResult;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Execute the task tool - create and run a subtask with its own agent
pub async fn execute_task(
    args: &str,
    body: &str,
    silent_mode: bool,
    _parent_agent_id: Option<AgentId>,
) -> ToolResult {
    // Parse arguments to extract task name, kind, and includes
    let (task_name, kind_name, includes) = parse_task_arguments(args);

    // Validate task instructions
    let task_instructions = body.trim();
    if task_instructions.is_empty() {
        let error_msg = "Error: Task requires instructions in the body".to_string();
        if !silent_mode {
            bprintln!(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Log task start information
    if !silent_mode {
        let kind_info = if let Some(kind) = &kind_name {
            format!("{}Using kind: {}{}\n", FORMAT_GRAY, kind, FORMAT_RESET)
        } else {
            String::new()
        };

        bprintln!(tool: "task",
            "\n{}🔄 Subtask Started:{} {}\n{}{}{}{}{}",
            FORMAT_BOLD,
            FORMAT_RESET,
            task_name,
            kind_info,
            FORMAT_GRAY,
            task_instructions,
            FORMAT_RESET,
            if !includes.is_empty() {
                let included_files = includes.join(", ");
                format!("\n{FORMAT_GRAY}Including files: {included_files}{FORMAT_RESET}")
            } else {
                String::new()
            }
        );
    }

    // Create config for the subtask agent
    let mut config = Config::new();

    // Set up the system prompt based on the kind
    let enabled_tools = prompts::ALL_TOOLS;
    let grammar = prompts::select_grammar_for_model("claude-3"); // Default to Claude grammar

    // Generate the system prompt
    if let Ok(system_prompt) = prompts::generate_system_prompt(
        enabled_tools,
        false,
        kind_name.as_deref(),
        grammar.clone(),
        Some(&config.disabled_tools), // Pass inherited disabled tools
    ) {
        config.system_prompt = Some(system_prompt);
    } else {
        let error_msg = format!("Failed to generate system prompt for task agent");
        if !silent_mode {
            bprintln!(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Set the kind parameter in the config
    config.kind = kind_name;

    // Create the subtask agent with a unique name
    let agent_name = format!("task_{task_name}");

    // Make a note of disabled tools for clarity in output
    if !config.disabled_tools.is_empty() && !silent_mode {
        bprintln!(
            "{}Task agent will have {} disabled tools: {}{}",
            FORMAT_GRAY,
            config.disabled_tools.len(),
            config.disabled_tools.join(", "),
            FORMAT_RESET
        );
    }

    let subtask_agent_id = match crate::agent::create_agent(agent_name, config) {
        Ok(id) => id,
        Err(e) => {
            let error_msg = format!("Failed to create task agent: {e}");
            if !silent_mode {
                bprintln!(error:"{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    };

    // Process file includes and combine with task instructions
    let combined_instructions = if !includes.is_empty() {
        let context_content = process_includes(&includes, silent_mode);
        if !context_content.is_empty() {
            // Combine file context with task instructions
            format!(
                "# File Context\n\n{}\n\n# Task Instructions\n\n{}",
                context_content, task_instructions
            )
        } else {
            task_instructions.to_string()
        }
    } else {
        task_instructions.to_string()
    };

    // Send the combined instructions (context + task) to the agent
    if let Err(e) = crate::agent::send_message(
        subtask_agent_id,
        AgentMessage::UserInput(combined_instructions),
    ) {
        let error_msg = format!("Failed to send task to agent: {e}");
        if !silent_mode {
            bprintln!(error:"{}", error_msg);
        }
        return ToolResult::error(error_msg);
    }

    // Wait for the agent to complete its task
    let result = wait_for_agent_completion(subtask_agent_id, silent_mode).await;

    // Log task completion
    if !silent_mode {
        bprintln!(tool: "task",
            "\n{}✅ Subtask Completed:{} {}\n{}",
            FORMAT_BOLD,
            FORMAT_RESET,
            task_name,
            result,
        );
    }

    // Return the result
    ToolResult::success(result)
}

/// Parse task arguments to extract task name, kind, and includes
fn parse_task_arguments(args: &str) -> (String, Option<String>, Vec<String>) {
    let args_string = args.trim().to_string();
    let mut kind_name = None;
    let mut includes = Vec::new();
    let mut task_name_parts = Vec::new();

    // Split the args by spaces to check for parameters
    let parts: Vec<&str> = args_string.split_whitespace().collect();

    for part in parts {
        if part.starts_with("kind=") {
            // Extract kind parameter
            if let Some(value) = part.strip_prefix("kind=") {
                kind_name = Some(value.to_string());
            }
        } else if part.starts_with("include=") {
            // Extract include parameter
            if let Some(value) = part.strip_prefix("include=") {
                includes.push(value.to_string());
            }
        } else {
            // This is part of the task name
            task_name_parts.push(part);
        }
    }

    // Reconstruct the task name from non-parameter parts
    let task_name = if task_name_parts.is_empty() {
        "unnamed_task".to_string()
    } else {
        task_name_parts.join(" ")
    };

    (task_name, kind_name, includes)
}

/// Process include files and return their contents
fn process_includes(includes: &[String], silent_mode: bool) -> String {
    let mut content = String::new();

    // Process each include file
    for include_pattern in includes {
        // Handle globbing
        match glob::glob(include_pattern) {
            Ok(paths) => {
                let mut found = false;

                for entry in paths {
                    match entry {
                        Ok(path) => {
                            found = true;
                            match read_file(&path) {
                                Ok(file_content) => {
                                    let file_name = path.display();
                                    // Add a clear file separator with markdown formatting
                                    content.push_str(&format!("\n## File: {}\n```\n", file_name));
                                    content.push_str(&file_content);
                                    // Close the code block and add spacing between files
                                    content.push_str("\n```\n\n");
                                }
                                Err(e) => {
                                    if !silent_mode {
                                        bprintln!(error:"Failed to read include file {}: {}", path.display(), e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if !silent_mode {
                                bprintln!(error:"Invalid path in glob pattern: {}", e);
                            }
                        }
                    }
                }

                if !found && !silent_mode {
                    bprintln!(error:"No files found matching pattern: {}", include_pattern);
                }
            }
            Err(e) => {
                if !silent_mode {
                    bprintln!(error:"Invalid glob pattern '{}': {}", include_pattern, e);
                }
            }
        }
    }

    content
}

/// Read a file and return its contents
fn read_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

/// Wait for agent to complete its task and return the final result
async fn wait_for_agent_completion(agent_id: AgentId, silent_mode: bool) -> String {
    let timeout = Duration::from_secs(300); // 5 minute timeout
    let start_time = Instant::now();
    let mut last_polling_time = Instant::now();
    let polling_interval = Duration::from_millis(500);

    let mut result = String::new();
    let mut done = false;

    // Keep checking the agent state until it's done or timeout
    while !done && start_time.elapsed() < timeout {
        // Only poll at the specified interval
        if last_polling_time.elapsed() >= polling_interval {
            last_polling_time = Instant::now();

            // Get the agent state
            let state = crate::agent::get_agent_state(agent_id).ok();

            match state {
                // Agent is done, get the result
                Some(AgentState::Done(response)) => {
                    // Extract the final response
                    if let Some(content) = response {
                        result = content;
                    } else {
                        // If no explicit done response, get the buffer content
                        let buffer_content = extract_final_output(agent_id);
                        result = buffer_content;
                    }

                    done = true;
                }

                // Agent is terminated, consider as done
                Some(AgentState::Terminated) => {
                    if !silent_mode {
                        bprintln!(warn: "Task agent was terminated before completion");
                    }
                    result = "Task was terminated before completion".to_string();
                    done = true;
                }

                // Other states - keep waiting
                _ => {}
            }
        }

        // Small sleep to avoid tight loop
        sleep(Duration::from_millis(50)).await;
    }

    // If we reached timeout
    if !done {
        if !silent_mode {
            bprintln!(warn: "Task timed out after {} seconds", timeout.as_secs());
        }
        result = format!("Task timed out after {} seconds", timeout.as_secs());

        // Terminate the agent
        let _ = crate::agent::terminate_agent(agent_id).await;
    }

    result
}

/// Extract the final output from the agent's buffer
fn extract_final_output(agent_id: AgentId) -> String {
    if let Ok(buffer) = crate::agent::get_agent_buffer(agent_id) {
        let lines = buffer.lines();

        // Simple approach: collect all meaningful content after the last user message
        // Find the last user message
        let mut last_user_idx = 0;
        for (i, line) in lines.iter().enumerate() {
            if line.content.starts_with(">") {
                last_user_idx = i;
            }
        }

        // Take everything after the last user message, filtering out just system messages and tool invocations
        let full_response = lines
            .iter()
            .skip(last_user_idx + 1)
            .filter(|line| {
                // Only filter out system messages (starting with 🤖)
                // We want to keep most content, including tool results
                !line.content.starts_with("🤖")
                    && !line.content.contains("Token usage:")
                    && !line.content.trim().is_empty()
            })
            .map(|line| line.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        if !full_response.is_empty() {
            return full_response;
        }

        // If somehow we got nothing, take a simpler approach - just get the last chunk of content
        return lines
            .iter()
            .rev()
            .take(20) // Take more lines to ensure we get substantial content
            .filter(|line| !line.content.starts_with("🤖"))
            .map(|line| line.content.clone())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
    }

    "Unable to retrieve agent output".to_string()
}
