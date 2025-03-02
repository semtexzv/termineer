//! Core Agent implementation for handling conversations with LLM backends
//!
//! This module contains the Agent struct and related functionality for
//! managing conversations, tool execution, and interactions with LLM backends.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::types::{AgentCommand, AgentId, AgentMessage, AgentReceiver, AgentState, StateSender};
use crate::config::Config;
use crate::constants::{TOOL_ERROR_END, TOOL_ERROR_START, TOOL_RESULT_END, TOOL_RESULT_START};
use crate::conversation::{is_done_tool, parse_assistant_response};
use crate::llm::{Backend, Content, Message, MessageInfo, TokenUsage};
use crate::tools::shell::{execute_shell, ShellOutput};
use crate::tools::InterruptData;
use crate::tools::ToolExecutor;

use tokio::sync::watch;

/// Result of sending a message, including whether further processing is needed
pub struct MessageResult {
    pub response: String,
    pub continue_processing: bool,
    /// Token usage statistics from the LLM response
    pub token_usage: Option<TokenUsage>,
}

/// Result of checking if the LLM wants to interrupt a streaming command
struct InterruptionCheck {
    pub interrupted: bool,
    pub reason: Option<String>,
}

/// Agent for interacting with LLM backends
pub struct Agent {
    /// Unique identifier for this agent
    pub id: AgentId,

    /// Human-readable name for this agent
    pub name: String,

    /// Configuration for this agent
    pub config: Config,

    /// LLM backend for generating responses
    pub llm: Box<dyn Backend>,

    /// Tool executor for handling tool invocations
    pub tool_executor: ToolExecutor,

    /// Conversation history
    pub conversation: Vec<Message>,

    /// Whether tools are restricted to read-only operations
    pub readonly_mode: bool,

    /// Stop sequences for LLM generation
    pub stop_sequences: Option<Vec<String>>,

    /// Cache points for conversation history
    pub cache_points: BTreeSet<usize>,

    /// Receiver for agent messages
    receiver: AgentReceiver,

    /// Sender of state updates
    sender: StateSender,

    /// Current state of the agent
    state: AgentState,

    /// Current shell interrupt data, if any
    interrupt_shell: Option<Arc<Mutex<InterruptData>>>,
}

impl Agent {
    /// Create a new agent with the given configuration and message receiver
    pub fn new(
        id: AgentId,
        name: String,
        mut config: Config,
        receiver: AgentReceiver,
        sender: StateSender,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize system prompt if not already set
        if config.system_prompt.is_none() {
            // Create appropriate tool options based on whether tools are enabled
            let tool_options = if config.enable_tools {
                crate::prompts::ToolDocOptions::default()
            } else {
                crate::prompts::ToolDocOptions::readonly()
            };
            
            // Generate the system prompt based on the minimal flag
            let prompt = if config.use_minimal_prompt {
                crate::prompts::generate_minimal_system_prompt(&tool_options)
            } else {
                crate::prompts::generate_system_prompt(&tool_options)
            };
            
            // Set the system prompt in the config
            config.system_prompt = Some(prompt);
        }

        // Create LLM backend using factory
        let llm = crate::llm::create_backend(&config)?;

        // Initialize tool executor (not readonly, not silent)
        let tool_executor = ToolExecutor::new(false, false);

        Ok(Self {
            id,
            name,
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
            receiver,
            sender,
            state: AgentState::Idle,
            interrupt_shell: None,
        })
    }
    fn set_state(&mut self, state: AgentState) {
        self.state = state.clone();
        self.sender.send(self.state.clone()).unwrap()
    }

    /// Run the agent, processing messages until terminated
    pub async fn run(mut self) {
        crate::bprintln!("Agent '{}' started", self.name);
        self.set_state(AgentState::Idle);

        // Main agent loop
        'main: loop {
            // Using tokio::select to handle multiple async operations
            tokio::select! {
                // Process incoming messages from UI
                msg = self.receiver.recv() => {
                    match msg {
                        Some(AgentMessage::UserInput(input)) => {
                            self.set_state(AgentState::Processing);
                            crate::bprintln!("Processing: {}", input);
                            if let Err(e) = self.process_input(&input).await {
                                crate::berror_println!("Error processing input: {}", e);
                            }
                            self.set_state(AgentState::Idle);
                        },
                        Some(AgentMessage::Interrupt) => {
                            self.handle_interrupt().await;
                        },
                        Some(AgentMessage::Terminate) => {
                            crate::bprintln!("Agent '{}' received terminate message", self.name);
                            break 'main; // Exit loop and terminate task
                        },
                        Some(AgentMessage::Command(cmd)) => {
                            self.handle_command(cmd).await;
                        },
                        None => {
                            // Channel closed - terminate
                            crate::bprintln!("Agent '{}' channel closed, terminating", self.name);
                            break 'main;
                        }
                    }
                },
                // We can add other select arms here if needed
            }
        }

        crate::bprintln!("Agent '{}' terminated", self.name);
    }

    /// Process a user input message
    async fn process_input(&mut self, input: &str) -> Result<(), Box<dyn std::error::Error>> {
        // If the agent is already in Done state, don't process further input
        if matches!(self.state, AgentState::Done) {
            crate::bprintln!("Agent has completed its task. Use /reset to start a new conversation.");
            return Ok(());
        }

        // First message from user
        let mut result = self.send_message(input).await?;
        let mut continue_processing = result.continue_processing;

        // Main agent loop - continue until we're done or interrupted
        while continue_processing {
            // Check if we're done (e.g., done tool was used)
            if self.is_done() {
                // Update state and exit the loop
                self.state = AgentState::Done;
                crate::bprintln!("Agent has completed its task.");
                break;
            }

            // Check if we've been interrupted or terminated
            if matches!(self.state, AgentState::Terminated) {
                crate::bprintln!("Agent processing was terminated.");
                break;
            }
            
            // If we're in Idle state but continue_processing is true, something is wrong
            // This is a safety check in case state transitions elsewhere are inconsistent
            if matches!(self.state, AgentState::Idle) {
                crate::bprintln!("Agent processing was interrupted (inconsistent state detected).");
                break;
            }

            // Keep processing - we need to be in Processing state to continue
            self.state = AgentState::Processing;
            
            // Try to continue to the next turn without user input
            match self.send_message("").await {
                Ok(next_result) => {
                    result = next_result;
                    continue_processing = result.continue_processing;
                }
                Err(e) => {
                    crate::berror_println!("Error during continued processing: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle an interrupt message
    async fn handle_interrupt(&mut self) {
        match &self.state {
            AgentState::RunningTool {
                tool,
                interruptible,
            } if *interruptible => {
                crate::bprintln!("Interrupting tool: {}", tool);

                // Interrupt shell execution
                if let Some(interrupt_data) = &self.interrupt_shell {
                    let mut data = interrupt_data.lock().unwrap();
                    data.interrupt("User requested interruption".to_string());
                    crate::bprintln!("Tool interrupted");
                }
            }
            AgentState::Processing => {
                crate::bprintln!("Interrupting processing (will take effect after current step)");
                // Just let the current step finish - we'll check state between steps
                self.state = AgentState::Idle;
            }
            _ => {
                crate::bprintln!("No interruptible operation in progress");
            }
        }
    }

    /// Handle a command message
    async fn handle_command(&mut self, cmd: AgentCommand) {
        match cmd {
            AgentCommand::SetModel(model) => {
                if let Err(e) = self.set_model(model.clone()) {
                    crate::berror_println!("Failed to set model to {}: {}", model, e);
                } else {
                    crate::bprintln!("Model set to {}", model);
                }
            }
            AgentCommand::EnableTools(enabled) => {
                self.enable_tools(enabled);
                crate::bprintln!("Tools {}abled", if enabled { "en" } else { "dis" });
            }
            AgentCommand::SetSystemPrompt(prompt) => {
                self.set_system_prompt(prompt);
                crate::bprintln!("System prompt updated");
            }
            AgentCommand::ResetConversation => {
                self.clear_conversation();
                crate::bprintln!("Conversation reset");
            }
        }
    }

    /// Check if the agent has completed its task
    fn is_done(&self) -> bool {
        self.conversation
            .last()
            .map(|m| {
                matches!(&m.info,
                    MessageInfo::ToolResult { tool_name } | MessageInfo::ToolError { tool_name }
                    if tool_name == "done"
                )
            })
            .unwrap_or(false)
    }

    /// Execute a shell command with streaming output and interruption capability
    async fn execute_streaming_shell(
        &mut self,
        command: &str,
    ) -> Result<MessageResult, Box<dyn std::error::Error>> {
        // Update state to running tool
        self.state = AgentState::RunningTool {
            tool: "shell".to_string(),
            interruptible: true,
        };

        // Extract the command from the tool content
        let command_str = command.to_string();
        let parts: Vec<&str> = command_str.trim().splitn(2, char::is_whitespace).collect();
        let cmd_args = if parts.len() > 1 {
            parts[1].to_string()
        } else {
            String::new()
        };

        // Create interrupt data for coordination
        let interrupt_data = Arc::new(Mutex::new(InterruptData::new()));
        // Store for potential external interruption
        self.interrupt_shell = Some(Arc::clone(&interrupt_data));

        // Execute shell command and get the output receiver
        let silent_mode = self.tool_executor.is_silent();
        let mut rx = execute_shell(&cmd_args, "", interrupt_data.clone(), silent_mode).await?;

        // Buffer to collect output for the conversation history
        let mut partial_output = String::new();

        let mut result_message = String::new();
        let mut success = true;

        // Flag to track if we're in the process of interrupting
        let mut interrupting = false;
        // Store the reason for interruption if provided
        let mut interruption_reason_str: Option<String> = None;

        // Track time between interruption checks (to prevent excessive API calls)
        let mut last_check_time = std::time::Instant::now();
        let min_check_interval = Duration::from_secs(10); // Check every 10 seconds - reduces API costs

        // Track if we have a partial tool result in the conversation
        let mut has_partial_result = false;

        // Loop to receive output and check for interruption
        loop {
            tokio::select! {
            // Process shell output
            output = rx.recv() => {
                match output {
                    Some(ShellOutput::Stdout(line)) => {
                        // Add to full output record
                        partial_output.push_str(&line);
                        partial_output.push('\n');

                        // Check for interruption every N seconds
                        if !interrupting && last_check_time.elapsed() > min_check_interval {
                            // Update last check time
                            last_check_time = std::time::Instant::now();

                            // Remove previous partial result if it exists
                            if has_partial_result {
                                self.conversation.pop();
                            }

                            // Create partial tool result message WITHOUT the ending tag
                            let partial_tool_result = format!(
                                "{}\n{}",
                                TOOL_RESULT_START, partial_output
                            );

                            // Mark this point in conversation as a cache point
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

                            // Send interruption check using the partial tool result
                            if let Ok(interruption_check) = self.check_for_interruption().await {
                                if interruption_check.interrupted {
                                    // Store the interruption reason if provided
                                    let reason = interruption_check.reason.unwrap_or_else(||
                                        "No specific reason provided".to_string()
                                    );

                                    // Set interrupt flag with reason
                                    {
                                        let mut data = interrupt_data.lock().unwrap();
                                        data.interrupt(reason.clone());
                                    }

                                    // Store the reason so we can use it in the final output
                                    interrupting = true;
                                    interruption_reason_str = Some(reason);
                                }
                            }
                        }
                    },
                    Some(ShellOutput::Stderr(line)) => {
                        // Add to full output record
                        partial_output.push_str(&line);
                        partial_output.push('\n');
                    },
                    Some(ShellOutput::Complete(tool_result)) => {
                        // Command completed, store results
                        success = tool_result.success;
                        result_message = tool_result.agent_output;
                        // Clear interrupt_shell as the command is done
                        self.interrupt_shell = None;
                        break;
                    },
                    None => {
                        // Channel closed, command must be done
                        // Clear interrupt_shell as the command is done
                        self.interrupt_shell = None;
                        break;
                    }
                }
            },

            // Add a timeout to avoid locking up AND check for messages
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                // Try to receive a message without blocking
                match self.receiver.try_recv() {
                    Ok(msg) => {
                        match msg {
                            AgentMessage::Interrupt => {
                                // Handle external interruption
                                let mut data = interrupt_data.lock().unwrap();
                                data.interrupt("User requested interruption".to_string());
                                interrupting = true;
                                interruption_reason_str = Some("User requested interruption".to_string());
                            },
                            AgentMessage::Terminate => {
                                // Handle termination request
                                let mut data = interrupt_data.lock().unwrap();
                                data.interrupt("Agent terminating".to_string());
                                self.state = AgentState::Terminated;
                                break;
                            },
                            _ => {
                                // Ignore other messages during shell execution
                            }
                        }
                    },
                    Err(_) => {
                        // No messages available or channel closed - continue
                    }
                }
            },

            }
        }

        // Properly finish the partial tool result if it exists
        if has_partial_result {
            // Remove the open partial result
            self.conversation.pop();
        }

        // Add a completion message to the result
        if interrupting {
            let reason = interruption_reason_str
                .as_ref()
                .map_or("Sufficient information gathered".to_string(), |r| r.clone());
            result_message = format!("{}\n\n[COMMAND INTERRUPTED: {}]", partial_output, reason);
            // When interrupted by LLM, this is NOT an error, it's a successful interruption
            success = true;
        } else {
            result_message = format!("{}\n\n[COMMAND COMPLETED SUCCESSFULLY]", partial_output);
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

        // Reset state only to Processing since we're continuing processing
        // (continue_processing is always true here)
        self.state = AgentState::Processing;

        // Return with continue_processing flag set to true
        Ok(MessageResult {
            response: result_message,
            continue_processing: true,
            token_usage: None,
        })
    }

    /// Sends a message to the LLM to check if it wants to interrupt the shell command
    async fn check_for_interruption(
        &mut self,
    ) -> Result<InterruptionCheck, Box<dyn std::error::Error>> {
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
        let temp_message = Message::text("user", interruption_check_message, MessageInfo::User);

        // Add message temporarily
        self.conversation.push(temp_message);

        // Use "</interrupt>" as stop sequence to allow content between tags
        let stop_sequences = vec!["</interrupt>".to_string(), "<continue/>".to_string()];

        // Allow 100 tokens for interruption reason
        let max_tokens_for_check = 100;

        let response = self
            .llm
            .send_message(
                &self.conversation,
                self.config.system_prompt.as_deref(), // Use the existing system prompt
                Some(&stop_sequences),
                None,
                Some(&current_cache_points),
                Some(max_tokens_for_check),
            )
            .await?;

        // Remove the temporary message
        self.conversation.pop();

        if response.stop_reason.as_deref() != Some("stop_sequence") {
            return Ok(InterruptionCheck {
                interrupted: false,
                reason: None,
            });
        }

        let stop_sequence = response.stop_sequence.unwrap();
        let content = response
            .content
            .iter()
            .map(|c| match c {
                crate::llm::Content::Text { text } => text.clone(),
                _ => String::new(),
            })
            .collect::<Vec<String>>()
            .join("");

        // Check if we're interrupting and extract the reason
        let (should_interrupt, reason) = if stop_sequence == "</interrupt>" {
            // Extract reason from <interrupt>reason</interrupt>
            let reason = if content.starts_with("<interrupt>") {
                content
                    .strip_prefix("<interrupt>")
                    .unwrap_or("")
                    .to_string()
            } else {
                "No specific reason provided".to_string()
            };

            (true, reason)
        } else {
            (false, String::new())
        };

        Ok(InterruptionCheck {
            interrupted: should_interrupt,
            reason: if should_interrupt { Some(reason) } else { None },
        })
    }

    /// Send a message to the LLM backend and process the response
    pub async fn send_message(
        &mut self,
        user_message: &str,
    ) -> Result<MessageResult, Box<dyn std::error::Error>> {
        // Add .autoswe file content to beginning of conversation if it hasn't been added yet
        if self.conversation.is_empty() && tokio::fs::try_exists(".autoswe").await? {
            let working = std::env::current_dir()?;
            let autoswe = tokio::fs::read_to_string(".autoswe").await?;
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

        let response = self
            .llm
            .send_message(
                &self.conversation,
                system_prompt,
                self.stop_sequences.as_deref(),
                thinking_budget,
                Some(&self.cache_points),
                None, // Use default max_tokens
            )
            .await?;

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
                    // Use the new macros for buffer printing
                    crate::bprintln!();
                    crate::conversation::print_token_stats(usage);
                    crate::bprintln!();
                }

                // Print the assistant's response using buffer-based printing
                crate::conversation::print_assistant_response(&assistant_message);
            }

            self.conversation.push(Message::text(
                "assistant",
                assistant_message.clone(),
                MessageInfo::Assistant,
            ));

            // If this is a regular response, set the state back to Idle
            // so the agent waits for the next user input
            self.state = AgentState::Idle;
            crate::bprintln!("Agent waiting for next user input.");

            return Ok(MessageResult {
                response: assistant_message,
                continue_processing: false, // Stop processing, wait for user input
                token_usage: response.usage,
            });
        }

        // At this point, we know we have a tool invocation
        let tool_name = parsed.tool_name.unwrap();
        let tool_content = parsed.tool_content.unwrap();

        // Log tool invocation to buffer
        crate::bprintln!("=== Tool invocation detected: {} ===", tool_name);
        crate::bprintln!("Tool content: {}", tool_content);

        // Display token stats before any other output (if not in silent mode)
        if !self.tool_executor.is_silent() {
            // Use buffer-based printing
            crate::bprintln!();
            if let Some(usage) = &response.usage {
                crate::conversation::print_token_stats(usage);
                crate::bprintln!();
            }
        }

        // Display the assistant's text before executing the tool
        if !parsed.text.is_empty() {
            crate::conversation::print_assistant_response(&parsed.text);
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
            let shell_result = self.execute_streaming_shell(&tool_content).await?;
            return Ok(shell_result);
        }

        // For other tools, update state
        let interruptible = false; // Only shell is interruptible for now
        self.state = AgentState::RunningTool {
            tool: tool_name.clone(),
            interruptible,
        };

        // Log before tool execution
        crate::bprintln!("=== Executing tool: {} ===", tool_name);
        
        // Execute the tool
        let tool_result = self.tool_executor.execute(&tool_content).await;
        
        // Log after tool execution
        crate::bprintln!("=== Tool execution completed ===");
        crate::bprintln!("Success: {}", tool_result.success);
        crate::bprintln!("Output length: {} characters", tool_result.agent_output.len());

        // Check if this is the "done" tool
        let is_done = is_done_tool(&tool_name);

        // Only reset to Idle if we're not going to continue processing
        // If this is not the "done" tool, we should stay in Processing state
        // to avoid being interrupted by the check in process_input()
        if !is_done {
            self.state = AgentState::Processing;  // Keep processing if continuing
        } else {
            self.state = AgentState::Idle;  // Reset to Idle if done
        }

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

        // Display tool output to user using the buffer system if not in silent mode
        crate::bprintln!("=== Preparing to display tool output to user ===");
        if !self.tool_executor.is_silent() {
            crate::bprintln!("Silent mode is OFF - should display output");
            
            // Display tool title
            crate::btool_println!(
                &tool_name,
                "Tool result from {}:",
                tool_name
            );
            
            // Display the actual tool output
            crate::bprintln!("------ TOOL OUTPUT START ------");
            crate::bprintln!("{}", tool_result.agent_output);
            crate::bprintln!("------ TOOL OUTPUT END ------");
            crate::bprintln!();
        } else {
            crate::bprintln!("Silent mode is ON - suppressing output display");
        }

        // Return value to use in the process flow
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
            if usage.input_tokens + usage.output_tokens > 300 {
                self.cache_here();
            }
        }

        // If this was the "done" tool, set state to Done and return with continue_processing=false
        if is_done {
            // Update state to Done
            self.state = AgentState::Done;
            crate::bprintln!("Agent has marked task as completed.");
            
            return Ok(MessageResult {
                response: result_for_response,
                continue_processing: false,
                token_usage: response.usage,
            });
        }

        // Return with continue_processing flag set to true to indicate tool processing should continue
        // The process_input loop will handle sending the next empty message
        Ok(MessageResult {
            response: result_for_response,
            continue_processing: true,
            token_usage: response.usage,
        })
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

    /// Add a cache point at the current conversation position
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
        // Reset state to Idle if it was Done
        if matches!(self.state, AgentState::Done) {
            self.state = AgentState::Idle;
            crate::bprintln!("Agent state reset to Idle.");
        }
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
    pub fn set_model(&mut self, model: String) -> Result<(), Box<dyn std::error::Error>> {
        self.config.model = model.clone();
        // Create new LLM provider with updated model using factory
        self.llm = crate::llm::create_backend(&self.config)?;
        // Reset cache points since model changed
        self.reset_cache_points();
        Ok(())
    }
}