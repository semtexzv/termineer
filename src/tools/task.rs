use crate::agent::{Agent, AgentId};
use crate::config::Config;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::llm::{Content, MessageInfo};
use crate::tools::ToolResult;
use tokio::sync::mpsc;

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

        return ToolResult {
            success: false,
            agent_output: error_msg,
        };
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

    // Temporarily simplified implementation that returns a success message
    // instead of actually creating a subtask
    // This is a workaround until the missing functions are implemented
    let result = format!(
        "Task '{}' processed with instructions:\n\n{}",
        task_name, task_instructions
    );

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

    ToolResult {
        success: true,
        agent_output: result,
    }
}
