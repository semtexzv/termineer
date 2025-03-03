use crate::agent::{Agent, AgentId, AgentMessage, AgentState};
use crate::agent::types::InterruptSignal;
use crate::config::Config;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::llm::{Content, MessageInfo};
use crate::tools::ToolResult;
use tokio::sync::{mpsc, watch};
use crate::prompts::{ToolDocOptions, generate_system_prompt};

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
                    clean_task_name
                        .push_str(task_name[model_pos + space_pos + 1 + end_pos..].trim());
                }
                task_name = clean_task_name;
            }
        }
    }

    let task_instructions = body.trim();

    if task_instructions.is_empty() {
        let error_msg = "Error: Task requires instructions in the body".to_string();

        if !silent_mode {
            // Use buffer instead of println
            crate::berror_println!("{}", error_msg);
        }

        return ToolResult::error(error_msg);
    }

    // Print model information if specified and not in silent mode
    if !silent_mode {
        // Use buffer-based printing with direct formatting
        if let Some(model) = &model_name {
            // With model info
            crate::btool_println!(
                "task",
                "\n{}🔄 Subtask Started:{} {}\n{}Using model: {}{}\n{}{}{}\n",
                FORMAT_BOLD,
                FORMAT_RESET,
                task_name,
                FORMAT_GRAY,
                model,
                FORMAT_RESET,
                FORMAT_GRAY,
                task_instructions,
                FORMAT_RESET
            );
        } else {
            // Without model info
            crate::btool_println!(
                "task",
                "\n{}🔄 Subtask Started:{} {}\n{}{}{}\n",
                FORMAT_BOLD,
                FORMAT_RESET,
                task_name,
                FORMAT_GRAY,
                task_instructions,
                FORMAT_RESET
            );
        }
    }
    
    // Create a configuration for the subtask
    let mut config = Config::new();
    
    // Set the model if specified
    if let Some(model) = model_name {
        config.model = model;
    }
    
    // Create message channels for communicating with the agent
    let (_sender, _receiver) = mpsc::channel::<AgentMessage>(100);
    // Create dedicated interrupt channel
    let (_interrupt_sender, _interrupt_receiver) = mpsc::channel::<InterruptSignal>(10);
    let (state_sender, _state_receiver) = watch::channel(AgentState::Idle);

    // Generate a task-specific ID
    let task_id = AgentId(999); // Use a dummy ID for now

    // Create a tool executor that's silent depending on the parent's silent mode
    // and set the system prompt
    let tool_options = ToolDocOptions::default();
    let system_prompt = generate_system_prompt(&tool_options);
    config.system_prompt = Some(system_prompt);
    
    // Create the agent
    let mut agent = match Agent::new(task_id, format!("task_{}", task_name), config, state_sender) {
        Ok(agent) => agent,
        Err(e) => {
            let error_msg = format!("Failed to create agent for task: {}", e);
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    };
    
    // Create a simple implementation that just adds the message to conversation
    // This avoids potential recursion with send_message
    // Add user message to conversation history
    agent.conversation.push(crate::llm::Message::text(
        "user",
        task_instructions.to_string(),
        crate::llm::MessageInfo::User,
    ));

    // Send the request using our LLM provider directly
    let system_prompt = agent.config.system_prompt.as_deref();
    let thinking_budget = Some(agent.config.thinking_budget);

    let response = match agent.llm.send_message(
        &agent.conversation,
        system_prompt,
        agent.stop_sequences.as_deref(),
        thinking_budget,
        None, // No cache points for tasks
        None, // Use default max_tokens
    ).await {
        Ok(res) => res,
        Err(e) => {
            let error_msg = format!("Error executing task: {}", e);
            if !silent_mode {
                crate::berror_println!("{}", error_msg);
            }
            return ToolResult::error(error_msg);
        }
    };

    // Extract content from response
    let mut assistant_response = String::new();
    for content in &response.content {
        if let crate::llm::Content::Text { text } = content {
            assistant_response.push_str(text);
        }
    }

    // Add the assistant's response to conversation
    agent.conversation.push(crate::llm::Message::text(
        "assistant",
        assistant_response.clone(),
        crate::llm::MessageInfo::Assistant,
    ));

    // Get all conversation messages
    let conversation = agent.conversation.clone();

    // Collect all assistant responses
    let mut result = String::new();
    for message in conversation {
        if message.role == "assistant" {
            // Skip tool calls and just get the text content
            if !matches!(message.info, MessageInfo::ToolCall { .. }) {
                if let Content::Text { text } = &message.content {
                    result.push_str(text);
                    result.push_str("\n\n");
                }
            }
        }
    }

    // If we have no result, use the assistant response directly
    if result.is_empty() {
        result = assistant_response;
    }

    // Print completion message if not in silent mode
    if !silent_mode {
        crate::btool_println!(
            "task",
            "\n{}✅ Subtask Completed:{} {}\n",
            FORMAT_BOLD,
            FORMAT_RESET,
            task_name
        );
    }

    ToolResult::success(result)
}
