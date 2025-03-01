//! Agent module for handling conversations with LLM backends
//!
//! This module contains the Agent struct and related functionality for
//! managing conversations, tool execution, and interactions with LLM backends.

use std::collections::BTreeSet;
use std::fs;

use crate::config::Config;
use crate::constants::{TOOL_ERROR_END, TOOL_ERROR_START, TOOL_RESULT_END, TOOL_RESULT_START};
use crate::conversation::{parse_assistant_response, print_assistant_response, print_token_stats, is_done_tool};
use crate::llm::{Backend, Content, Message, MessageInfo, TokenUsage};
use crate::prompts::{generate_minimal_system_prompt, ToolDocOptions};
use crate::tools::ToolExecutor;

/// Result of sending a message, including whether further processing is needed
pub struct MessageResult {
    pub response: String,
    pub continue_processing: bool,
    /// Token usage statistics from the LLM response
    /// Not directly accessed but kept for future usage analytics
    #[allow(dead_code)]
    pub token_usage: Option<TokenUsage>,
}

/// Agent for interacting with LLM backends
pub struct Agent {
    pub config: Config,
    pub llm: Box<dyn Backend>,
    pub tool_executor: ToolExecutor,
    pub conversation: Vec<Message>,
    pub readonly_mode: bool,
    pub stop_sequences: Option<Vec<String>>,
    pub cache_points: BTreeSet<usize>,
}

impl Agent {
    pub fn new(config: Config) -> Self {
        // Create LLM backend using factory
        let llm = crate::llm::create_backend(&config)
            .expect("Failed to create LLM backend");

        let tool_executor = ToolExecutor::new(false, false); // not readonly, not silent

        Self {
            config,
            llm,
            tool_executor,
            conversation: Vec::new(),
            readonly_mode: false,
            stop_sequences: Some(vec![
                TOOL_RESULT_START.to_string(),
                TOOL_ERROR_START.to_string(),
            ]),
            cache_points: BTreeSet::new(),
        }
    }

    /// Reset cache points - needed when system prompt changes or
    /// when messages before cache points are modified/removed
    pub fn reset_cache_points(&mut self) {
        self.cache_points.clear();
        // Only set a cache point at the last message if there are any messages
        if !self.conversation.is_empty() {
            self.cache_points.insert(self.conversation.len() - 1);
        }
    }

    pub fn cache_here(&mut self) {
        self.cache_points.insert(self.conversation.len() - 1);
        if self.cache_points.len() > 3 {
            let pos = *self.cache_points.iter().next().unwrap();
            self.cache_points.remove(&pos);
        }
    }

    /// Clear the conversation history
    pub fn clear_conversation(&mut self) {
        self.conversation.clear();
        // Clear all cache points when conversation is cleared
        self.cache_points.clear();
    }

    /// Set the system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.config.system_prompt = Some(prompt);
        // System prompt change invalidates cache
        self.reset_cache_points();
    }

    /// Enable or disable tools
    pub fn enable_tools(&mut self, enabled: bool) {
        self.config.enable_tools = enabled;
    }

    /// Set the thinking budget
    pub fn set_thinking_budget(&mut self, budget: usize) {
        self.config.thinking_budget = budget;
    }

    /// Set the model to use
    pub fn set_model(&mut self, model: String) {
        self.config.model = model.clone();
        // Create new LLM provider with updated model using factory
        self.llm = crate::llm::create_backend(&self.config)
            .expect("Failed to create LLM backend");
        // Reset cache points since model changed
        self.reset_cache_points();
    }

    /// Create a subagent configuration with read-only tools
    /// Returns a new Agent instance configured for subagent use
    /// Optional model parameter allows specifying which model to use for the subagent
    pub fn create_subagent_for_task(task_name: &str, model: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        // Create a new config for the subagent with default model
        let mut subagent_config = crate::config::Config::new();
        
        // Set the model if provided
        if let Some(ref model_name) = model {
            subagent_config.model = model_name.clone();
        }

        // Use minimal prompt to save tokens
        let options = ToolDocOptions::readonly();
        let minimal_prompt = generate_minimal_system_prompt(&options);

        // Create task-specific system prompt
        let subagent_system_prompt = format!(
            "{}\n\nYou are a subagent created to complete a specific task.\n\
            Your task is: \"{}\"\n\
            Complete this task thoroughly and return your results using the done tool when finished.\n\
            You must put your final results in the done tool.\n\
            Note: You are in read-only mode and can only use shell, read, fetch, and done tools.\n\n\
            Important guidelines for research:\n\
            - Spend at least 30% of your effort on thorough exploration before drawing conclusions\n\
            - Analyze from multiple perspectives and consider alternative interpretations\n\
            - Document your reasoning process and how you arrived at your conclusions\n\
            - Challenge your initial assumptions and verify your understanding with evidence\n\
            - Use systematic approaches when exploring codebases or documentation", 
            minimal_prompt, task_name
        );

        subagent_config.system_prompt = Some(subagent_system_prompt);

        // Create an LLM backend for the subagent
        let model_str = model.as_deref();
        let llm = crate::llm::create_backend_for_task(model_str)
            .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

        // Create tool executor (readonly and silent)
        let tool_executor = ToolExecutor::new(true, true);

        // Create subagent directly
        let mut subagent = Self {
            config: subagent_config,
            llm,
            tool_executor,
            conversation: Vec::new(),
            readonly_mode: true,
            stop_sequences: Some(vec![
                TOOL_RESULT_START.to_string(),
                TOOL_ERROR_START.to_string(),
            ]),
            cache_points: BTreeSet::new(),
        };

        subagent.enable_tools(true);

        Ok(subagent)
    }

    /// Send a message to the LLM backend and process the response
    pub fn send_message(
        &mut self,
        user_message: &str,
    ) -> Result<MessageResult, Box<dyn std::error::Error>> {
        // Add .autoswe file content to beginning of conversation if it hasn't been added yet
        if self.conversation.is_empty() && fs::exists(".autoswe")? {
            let working = std::env::current_dir()?;
            let autoswe = fs::read_to_string(".autoswe")?;
            let content = format!(
                "# You're currently working in this directory:\n```\n{}\n```\n# Project information:\n{}",
                working.to_str().unwrap_or("unknown"),
                autoswe
            );

            // Insert as a regular user message at the beginning
            self.conversation
                .push(Message::text("user", content, MessageInfo::User));
        }

        if !user_message.is_empty() {
            // Add user message to conversation history
            self.conversation.push(Message::text(
                "user",
                user_message.to_string(),
                MessageInfo::User,
            ));
        }

        // Send the request using our LLM provider
        let system_prompt = self.config.system_prompt.as_deref();
        let thinking_budget = Some(self.config.thinking_budget);

        let response = self.llm.send_message(
            &self.conversation,
            system_prompt,
            self.stop_sequences.as_deref(),
            thinking_budget,
            Some(&self.cache_points),
        )?;

        
        // Extract content from response
        let mut assistant_message = String::new();
        for content in &response.content {
            if let Content::Text { text } = content {
                assistant_message.push_str(text);
            }
        }

        // Parse the assistant's response
        let parsed = parse_assistant_response(&assistant_message);
        
        // If tools are not enabled, or no tool was found, handle as a regular response
        if !self.config.enable_tools || parsed.tool_name.is_none() {
            self.conversation.push(Message::text(
                "assistant",
                assistant_message.clone(),
                MessageInfo::Assistant,
            ));

            return Ok(MessageResult {
                response: assistant_message,
                continue_processing: false,
                token_usage: response.usage,
            });
        }
        
        // At this point, we know we have a tool invocation
        let tool_name = parsed.tool_name.unwrap();
        let tool_content = parsed.tool_content.unwrap();
        
        // Display token stats before any other output (if not in silent mode)
        if !self.tool_executor.is_silent() {
            println!();
            if let Some(usage) = &response.usage {
                print_token_stats(usage);
                println!()
            }
        }
        
        // Display the assistant's text before executing the tool
        if !parsed.text.is_empty() {
            print_assistant_response(&parsed.text);
        }

        // Everything before and including the tool invocation (we need this for conversation history)
        let full_assistant_message = assistant_message.clone();
        
        // Execute the tool using our tool executor
        let tool_result = self.tool_executor.execute(&tool_content);
        
        // Check if this is the "done" tool
        let is_done_tool = is_done_tool(&tool_name);

        // Format the agent response with appropriate delimiters
        let agent_response = if tool_result.success {
            format!(
                "{}\n{}\n{}",
                TOOL_RESULT_START, tool_result.agent_output, TOOL_RESULT_END
            )
        } else {
            format!(
                "{}\n{}\n{}",
                TOOL_ERROR_START, tool_result.agent_output, TOOL_ERROR_END
            )
        };
        
        // Return value to use in the process_user_query flow
        let result_for_response = tool_result.agent_output.clone();

        // Add the assistant's response (with tool invocation) to conversation history
        self.conversation.push(Message::text(
            "assistant",
            full_assistant_message,
            MessageInfo::ToolCall {
                tool_name: tool_name.clone(),
            },
        ));

        // Add the agent_response to the conversation history (for the LLM to see)
        // Determine the MessageInfo based on whether it was a successful tool execution
        let message_info = if tool_result.success {
            MessageInfo::ToolResult {
                tool_name: tool_name.clone(),
            }
        } else {
            MessageInfo::ToolError {
                tool_name: tool_name.clone(),
            }
        };
        
        self.conversation
            .push(Message::text("user", agent_response, message_info));
        
        // Cache frequently.
        if let Some(usage) = &response.usage {
            if usage.input_tokens > 300 {
                self.cache_here();
            }
        }

        // If this was the "done" tool, return with continue_processing=false
        if is_done_tool {
            return Ok(MessageResult {
                response: result_for_response,
                continue_processing: false,
                token_usage: response.usage,
            });
        }

        // Return with continue_processing flag set to true to indicate tool processing should continue
        Ok(MessageResult {
            response: result_for_response,
            continue_processing: true,
            token_usage: response.usage,
        })
    }
}

/// Process a user query, handling tool calls and multi-turn interactions
///
/// When silent_mode is true, no console output is produced (for subagents)
/// Returns a tuple of (task_completed, last_response)
pub fn process_user_query(
    client: &mut Agent,
    user_input: &str,
    silent_mode: bool,
) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let mut message_sent = false;

    // Loop until we get a "done" tool or no further processing is needed
    loop {
        // Send the message to the LLM
        let message_result = if !message_sent {
            // First message is the user's input
            message_sent = true;
            client.send_message(user_input)
        } else {
            // Subsequent messages are empty - The assistant will continue with tool output
            client.send_message("")
        };

        match message_result {
            Ok(result) => {

                // Check for task completion by looking for done tool
                let task_completed = client.conversation.last().map(|m|
                    matches!(&m.info,
                        MessageInfo::ToolResult { tool_name } | MessageInfo::ToolError { tool_name } if tool_name == "done"
                    )
                ).unwrap_or(false);

                // Check if we should continue processing
                if !result.continue_processing || task_completed {
                    return Ok((task_completed,  result.response.clone()));
                }
            }
            Err(err) => {
                if !silent_mode {
                    println!("\nError: {}\n", err);
                }
                return Err(err);
            }
        }
    }
}