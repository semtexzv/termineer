//! Agent module for handling conversations with LLM backends
//!
//! This module contains the Agent struct and related functionality for
//! managing conversations, tool execution, and interactions with LLM backends.

use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal;

use crate::config::Config;
use crate::constants::{TOOL_ERROR_END, TOOL_ERROR_START, TOOL_RESULT_END, TOOL_RESULT_START};
use crate::conversation::{parse_assistant_response, print_assistant_response, print_token_stats, is_done_tool};
use crate::llm::{Backend, Content, Message, MessageInfo, TokenUsage};
use crate::prompts::{generate_minimal_system_prompt, ToolDocOptions};
use crate::tools::ToolExecutor;
use crate::tools::shell_async::{execute_shell_async, ToolMessage};

/// Result of sending a message, including whether further processing is needed
pub struct MessageResult {
    pub response: String,
    pub continue_processing: bool,
    /// Token usage statistics from the LLM response
    /// Not directly accessed but kept for future usage analytics
    #[allow(dead_code)]
    pub token_usage: Option<TokenUsage>,
}

/// Result of checking if the LLM wants to interrupt a streaming command
struct InterruptionCheck {
    pub interrupted: bool,
    pub reason: Option<String>,
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
    /// Execute a shell command with streaming output and interruption capability
    fn execute_streaming_shell(&mut self, command: &str) -> Result<MessageResult, Box<dyn std::error::Error>> {
        use std::sync::mpsc::TryRecvError;
        
        // Extract the command from the tool content and make it owned data (String)
        let command_str = command.to_string();
        let parts: Vec<&str> = command_str.trim().splitn(2, char::is_whitespace).collect();
        let cmd_args = if parts.len() > 1 { parts[1].to_string() } else { String::new() };
        
        // Flag to track if the command should be interrupted
        let interrupt_flag = Arc::new(Mutex::new(false));
        let interrupt_flag_thread = Arc::clone(&interrupt_flag);
        let interrupt_flag_main = Arc::clone(&interrupt_flag);
        
        // Setup channel for streaming output
        let (tx, rx) = std::sync::mpsc::channel();
        
        // Start the shell command in a separate thread
        let silent_mode = self.tool_executor.is_silent();
        let cmd_thread = thread::spawn(move || {
            execute_shell_async(cmd_args.clone(), tx, interrupt_flag_thread, silent_mode);
        });
        
        // Buffer to collect output for the conversation history
        let mut full_output = String::new();
        let mut result_message = String::new();
        let mut success = true;
        
        // Flag to track if we're in the process of interrupting
        let mut interrupting = false;
        // Store the reason for interruption if provided
        let mut interruption_reason_str: Option<String> = None;
        
        // Buffer of lines to reduce the frequency of LLM interruption checks
        let mut line_buffer = String::new();
        
        // Track time between interruption checks (to prevent excessive API calls)
        let mut last_check_time = std::time::Instant::now();
        let min_check_interval = std::time::Duration::from_secs(10); // Check every 10 seconds - reduces API costs
        
        // Track if we have a partial tool result in the conversation
        let mut has_partial_result = false;
        
        if !self.tool_executor.is_silent() {
            println!("🔄 Shell execution started with interrupt capability (Press Ctrl+C to interrupt)");
        }

        // Setup for handling keyboard interrupts from user
        let raw_mode_result = if !silent_mode {
            terminal::enable_raw_mode()
        } else {
            // Don't enable raw mode in silent mode
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Silent mode"))
        };
        let raw_mode_enabled = raw_mode_result.is_ok();
        
        // Loop to receive output and check for interruption
        loop {
            // Check for keyboard input if raw mode is enabled (with shorter poll time)
            if raw_mode_enabled && event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                        // User pressed Ctrl+C - just set flags, don't change terminal mode or print
                        // This prevents race conditions with subprocess output
                        
                        // Set interrupt flag
                        {
                            let mut flag = interrupt_flag_main.lock().unwrap();
                            *flag = true;
                        }
                        
                        // Store the reason as user interruption
                        interrupting = true;
                        interruption_reason_str = Some("User interrupted command with Ctrl+C".to_string());
                        
                        // Don't print anything yet - will print after terminal mode is restored
                        // Don't disable raw mode yet - will cause inconsistent output formatting
                    }
                }
            }
            
            match rx.try_recv() {
                Ok(ToolMessage::Line(line)) => {
                    // Add to full output record
                    full_output.push_str(&line);
                    full_output.push('\n');
                    
                    // Add to line buffer for batched interruption checks
                    line_buffer.push_str(&line);
                    line_buffer.push('\n');
                    
                    // Check for interruption every 2 seconds
                    if !interrupting && !line_buffer.is_empty() && last_check_time.elapsed() > min_check_interval {
                        // Update last check time
                        last_check_time = std::time::Instant::now();
                        
                        // Remove previous partial result if it exists
                        if has_partial_result {
                            self.conversation.pop();
                        }
                        
                        // Format the current partial result - WITHOUT ending tag to indicate it's in progress
                        let stdout_line_count = full_output.lines().count();
                        let partial_output = format!(
                            "STDOUT (lines: {})\n{}\n\n[THIS IS A PARTIAL RESULT: Shell command is still running. The LLM is currently checking if it should interrupt the process.]",
                            stdout_line_count, full_output
                        );
                        
                        // Create partial tool result message WITHOUT the ending tag
                        let partial_tool_result = format!(
                            "{}\n{}",
                            TOOL_RESULT_START, partial_output
                        );
                        
                        // Mark this point in conversation as a cache point (before we add the partial result)
                        self.cache_here();
                        
                        // Add partial result to conversation
                        self.conversation.push(Message::text(
                            "user", 
                            partial_tool_result,
                            MessageInfo::ToolResult {
                                tool_name: "shell".to_string(),
                            }
                        ));
                        
                        has_partial_result = true;
                        
                        // Send interruption check using the partial tool result already in conversation
                        if let Ok(interruption_check) = self.check_for_interruption() {
                            if interruption_check.interrupted {
                                // Store the interruption reason if provided
                                let interruption_reason = interruption_check.reason.unwrap_or_else(|| 
                                    "No specific reason provided".to_string()
                                );
                                
                                // Log the interruption more concisely
                                if !self.tool_executor.is_silent() {
                                    println!("🛑 Interrupting: {}", interruption_reason);
                                    println!("🔄 Setting interrupt flag");
                                }
                                
                                // Set interrupt flag to signal shell command to stop
                                {
                                    let mut flag = interrupt_flag_main.lock().unwrap();
                                    *flag = true;
                                }
                                
                                // Store the reason so we can use it in the final output
                                interrupting = true;
                                interruption_reason_str = Some(interruption_reason);
                                
                                // Debug verification
                                if !self.tool_executor.is_silent() {
                                    let flag_value = *interrupt_flag_main.lock().unwrap();
                                    println!("✅ Interrupt flag is now: {}", flag_value);
                                }
                            }
                            
                            // Reset buffer after check
                            line_buffer.clear();
                        }
                    }
                    
                    // Continue receiving output
                    continue;
                },
                Ok(ToolMessage::Complete(tool_result)) => {
                    // Command completed, store results
                    success = tool_result.success;
                    result_message = tool_result.agent_output;
                    
                    // Set the result message but don't add a note yet
                    // We'll add completion status when finalizing the tool result
                    
                    break;
                },
                Err(TryRecvError::Empty) => {
                    // No message available right now, wait a bit
                    thread::sleep(std::time::Duration::from_millis(50));
                },
                Err(TryRecvError::Disconnected) => {
                    // Channel disconnected, command must be done
                    break;
                }
            }
        }
        
        // Wait for command thread to finish first - ensures all output is complete
        let _ = cmd_thread.join();
        
        // Now that all output processing is complete, restore terminal mode
        if raw_mode_enabled {
            let _ = terminal::disable_raw_mode();
            
            // Safe to print interruption message now that terminal mode is normalized
            // and all subprocess output has been processed
            if interrupting && !silent_mode && interruption_reason_str.as_ref().map_or(false, |r| r.contains("Ctrl+C")) {
                println!("🛑 User interrupted command with Ctrl+C");
            }
        }
        
        // Properly finish the partial tool result if it exists
        if has_partial_result {
            // Remove the open partial result
            self.conversation.pop();
            
            // Add a completion message to the result
            if interrupting {
                let reason = interruption_reason_str
                    .as_ref()
                    .map_or(
                        "Sufficient information gathered".to_string(),
                        |r| r.clone()
                    );
                result_message = format!("{}\n\n[COMMAND INTERRUPTED: {}]", full_output, reason);
                // When interrupted by LLM, this is NOT an error, it's a successful interruption
                success = true;
            } else {
                result_message = format!("{}\n\n[COMMAND COMPLETED SUCCESSFULLY]", full_output);
            }
        }
        
        // Format the shell output with appropriate delimiters
        // Note: Interruption is NOT an error, so we use TOOL_RESULT for it
        let agent_response = if success || interrupting {
            format!(
                "{}\n{}\n{}",
                TOOL_RESULT_START, result_message, TOOL_RESULT_END
            )
        } else {
            format!(
                "{}\n{}\n{}",
                TOOL_ERROR_START, result_message, TOOL_ERROR_END
            )
        };
        
        // Add the agent_response to the conversation history
        // Interruption should be treated as a successful result
        let message_info = if success || interrupting {
            MessageInfo::ToolResult {
                tool_name: "shell".to_string(),
            }
        } else {
            MessageInfo::ToolError {
                tool_name: "shell".to_string(),
            }
        };
        
        self.conversation
            .push(Message::text("user", agent_response, message_info));
        
        // Return with continue_processing flag set to true
        Ok(MessageResult {
            response: result_message,
            continue_processing: true,
            token_usage: None,
        })
    }
    
    /// Sends a message to the LLM to check if it wants to interrupt the shell command
    /// based on the partial tool result already in the conversation
    fn check_for_interruption(&mut self) -> Result<InterruptionCheck, Box<dyn std::error::Error>> {
        // Save current cache points
        let current_cache_points = self.cache_points.clone();
        
        // Create a shorter prompt for the interruption check
        let interruption_check_message = format!(
            "========== COMMAND INTERRUPTION CHECK ==========\n\
            Evaluate if this command should be interrupted based on its current output.\n\
            \n\
            Interrupt if:\n\
            - You have enough information to proceed\n\
            - The output is repetitive or redundant\n\
            - Errors indicate the command won't recover\n\
            \n\
            RESPONSE FORMAT:\n\
            - To continue: '<continue/>'\n\
            - To interrupt: '<interrupt>ONE SENTENCE REASON</interrupt>'\n\
            \n\
            If interrupting, provide exactly ONE SENTENCE explaining why.\n\
            Your decision:"
        );
        
        // Create a temporary message to add to conversation
        let temp_message = Message::text(
            "user",
            interruption_check_message,
            MessageInfo::User,
        );
        
        // Add message temporarily
        self.conversation.push(temp_message);

        // Use "</interrupt>" as stop sequence to allow content between tags
        let stop_sequences = vec!["</interrupt>".to_string(), "<continue/>".to_string()];
        
        // Allow 100 tokens for interruption reason
        let max_tokens_for_check = 100;

        let response = self.llm.send_message(
            &self.conversation,
            self.config.system_prompt.as_deref(), // Use the existing system prompt
            Some(&stop_sequences),
            None,
            Some(&current_cache_points),
            Some(max_tokens_for_check),
        ).unwrap();
        
        // Remove the temporary message
        self.conversation.pop();

        if response.stop_reason.as_deref() != Some("stop_sequence") {
            return Ok(InterruptionCheck { 
                interrupted: false,
                reason: None 
            });
        }
        
        let stop_sequence = response.stop_sequence.unwrap();
        let content = response.content.iter().map(|c| match c {
            crate::llm::Content::Text { text } => text.clone(),
            _ => String::new(),
        }).collect::<Vec<String>>().join("");
        
        // Check if we're interrupting and extract the reason
        let (should_interrupt, reason) = if stop_sequence == "</interrupt>" {
            // Extract reason from <interrupt>reason</interrupt>
            let reason = if content.starts_with("<interrupt>") {
                content.strip_prefix("<interrupt>").unwrap_or("").to_string()
            } else {
                "No specific reason provided".to_string()
            };
            
            (true, reason)
        } else {
            (false, String::new())
        };
        
        // Only log if interrupting - otherwise stay invisible to user
        if should_interrupt && !self.tool_executor.is_silent() {
            println!("🔍 Decision: INTERRUPT - {}", reason);
        }
        
        Ok(InterruptionCheck {
            interrupted: should_interrupt,
            reason: if should_interrupt { Some(reason) } else { None },
        })
    }
    
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
            None, // Use default max_tokens
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
            // In interactive mode, print the response here
            if !self.tool_executor.is_silent() {
                // Print token usage stats if available
                if let Some(usage) = &response.usage {
                    println!();
                    print_token_stats(usage);
                    println!();
                }
                
                // Print the assistant's response
                print_assistant_response(&assistant_message);
            }
            
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
        
        // Add the assistant's response (with tool invocation) to conversation history
        self.conversation.push(Message::text(
            "assistant",
            full_assistant_message,
            MessageInfo::ToolCall {
                tool_name: tool_name.clone(),
            },
        ));
        
        // Special handling for shell tool to support streaming and interruption
        if tool_name == "shell" {
            // Execute shell command with streaming output and interruption capability
            let shell_result = self.execute_streaming_shell(&tool_content)?;
            return Ok(shell_result);
        }
        
        // For other tools, execute normally using our tool executor
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