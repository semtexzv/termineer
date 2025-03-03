//! Core Agent implementation for handling conversations with LLM backends
//!
//! This module contains the Agent struct and related functionality for
//! managing conversations, tool execution, and interactions with LLM backends.

use std::collections::BTreeSet;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::interrupt::{spawn_interrupt_monitor, InterruptCoordinator};
use super::types::{
    AgentCommand, AgentId, AgentMessage, AgentReceiver, AgentState, InterruptReceiver, StateSender,
};
use crate::ansi_converter::strip_ansi_sequences;
use crate::config::Config;
use crate::constants::{
    TOOL_ERROR_END, TOOL_ERROR_START_PREFIX, TOOL_RESULT_END, TOOL_RESULT_START_PREFIX,
};
use crate::conversation::{
    sanitize_conversation, truncate_conversation, TruncationConfig,
};
use crate::llm::{Backend, Content, Message, MessageInfo, TokenUsage};
use crate::prompts::Grammar;
use crate::prompts::OldGrammar;
use crate::tools::shell::{execute_shell, ShellOutput};
use crate::tools::InterruptData;
use crate::tools::ToolExecutor;

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

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
    
    /// Grammar for formatting tools and parsing responses
    pub grammar: Box<dyn Grammar>,

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

    /// Configuration for conversation truncation
    truncation_config: TruncationConfig,


    /// Sender of state updates
    sender: StateSender,

    /// Current state of the agent
    state: AgentState,

    /// Counter for tool invocations, used for indexing tool results
    tool_invocation_counter: usize,
}

impl Agent {
    /// Create a new agent with the given configuration and communication channels
    pub fn new(
        id: AgentId,
        name: String,
        mut config: Config,
        sender: StateSender,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Initialize default grammar
        let grammar: Box<dyn Grammar> = Box::new(OldGrammar {});
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
                crate::prompts::generate_minimal_system_prompt(&tool_options, grammar.deref())
            } else {
                crate::prompts::generate_system_prompt(&tool_options, grammar.deref())
            };

            // Set the system prompt in the config
            config.system_prompt = Some(prompt);
        }

        // Create LLM backend using factory
        let llm = crate::llm::create_backend(&config).map_err(|e| {
            Box::<dyn std::error::Error + Send + Sync>::from(format!(
                "Failed to create LLM backend: {}",
                e
            ))
        })?;

        // Initialize tool executor (not readonly, not silent)
        // Note: Agent manager will be set later in the run method
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
                grammar.stop_sequences().done_stop_sequence.to_string(),
                grammar.stop_sequences().error_stop_sequence.to_string(),
            ]),
            cache_points: BTreeSet::new(),
            truncation_config: TruncationConfig::default(),
            sender,
            state: AgentState::Idle,
            tool_invocation_counter: 0,
            grammar,
        })
    }
    fn set_state(&mut self, state: AgentState) {
        self.state = state.clone();
        self.sender.send(self.state.clone()).unwrap()
    }

    /// Run the agent, processing messages until terminated
    pub async fn run(
        mut self,
        mut agent_receiver: AgentReceiver,
        interrupt_receiver: InterruptReceiver,
    ) {
        use crate::GLOBAL_AGENT_MANAGER;
        
        crate::bprintln!(
            "🤖 {}Agent{} '{}' started",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            self.name
        );
        self.set_state(AgentState::Idle);

        // Setup interrupt coordination channels
        let (agent_interrupt_tx, mut agent_interrupt_rx) = mpsc::channel(10);
        let coordinator = Arc::new(InterruptCoordinator::new(agent_interrupt_tx));
        let _interrupt_monitor = spawn_interrupt_monitor(coordinator.clone(), interrupt_receiver);
        
        // Set up the tool executor with this agent's ID (using global agent manager)
        self.tool_executor = ToolExecutor::with_agent_manager(
            false, 
            false,
            self.id
        );

        // Main agent loop
        'main: loop {
            // Store the current state to make borrow checker happy
            let current_state = self.state.clone();

            tokio::select! {
                biased;

                // Handle any possible interrupts (routed to us by the monitor)
                // This has highest priority (biased select)
                _ = agent_interrupt_rx.recv() => {
                    // Add interrupt message to conversation
                    self.conversation.push(Message::text(
                        "user",
                        "*Processing was interrupted by user*".to_string(),
                        MessageInfo::User,
                    ));
                    // Display with bold dark blue formatting
                    crate::bprintln!("{}{}*Processing was interrupted by user*{}",
                        crate::constants::FORMAT_BOLD,
                        crate::constants::FORMAT_BLUE,
                        crate::constants::FORMAT_RESET);
                    self.set_state(AgentState::Idle);
                    continue;
                }

                // Call LLM (Interruptible) - only when in Processing state
                result = self.send_message(&coordinator), if matches!(current_state, AgentState::Processing) => {
                    match result {
                        Ok(result) => {
                            if !result.continue_processing {
                                crate::bprintln!("✅ {}Agent{} has completed its task.",
                                    crate::constants::FORMAT_BOLD,
                                    crate::constants::FORMAT_RESET);
                                self.set_state(AgentState::Done(Some(result.response)))
                            }
                        },
                        Err(e) => {
                            crate::berror_println!("Error during processing: {}", e);
                            self.set_state(AgentState::Idle);
                        }
                    }

                    // Process any pending messages that arrived during LLM processing
                    'queue: loop {
                        match agent_receiver.try_recv() {
                            Ok(msg) => {
                                self.handle_message(msg).await;

                                // Check if we've been terminated after handling message
                                if matches!(self.state, AgentState::Terminated) {
                                    break 'main;
                                }
                            },
                            Err(TryRecvError::Empty) => {
                                // We've processed all messages, continue
                                break 'queue;
                            }
                            Err(TryRecvError::Disconnected) => {
                                crate::bprintln!("{}Agent{} '{}' channel closed, terminating",
                                    crate::constants::FORMAT_BOLD,
                                    crate::constants::FORMAT_RESET,
                                    self.name);
                                break 'main;
                            }
                        }
                    }
                }

                // Wait for and process messages when idle or done
                msg = agent_receiver.recv(), if matches!(current_state, AgentState::Done(_) | AgentState::Idle) => {
                    match msg {
                        Some(message) => {
                            self.handle_message(message).await;

                            // Check if we've been terminated after handling message
                            if matches!(self.state, AgentState::Terminated) {
                                break 'main;
                            }
                        },
                        None => {
                            // Channel closed, terminate agent
                            crate::bprintln!("{}Agent{} '{}' channel closed, terminating",
                                    crate::constants::FORMAT_BOLD,
                                    crate::constants::FORMAT_RESET,
                                    self.name);
                            break 'main;
                        }
                    }
                    // Process any pending messages that arrived during LLM processing
                    'queue: loop {
                        match agent_receiver.try_recv() {
                            Ok(msg) => {
                                self.handle_message(msg).await;

                                // Check if we've been terminated after handling message
                                if matches!(self.state, AgentState::Terminated) {
                                    break 'main;
                                }
                            },
                            Err(TryRecvError::Empty) => {
                                // We've processed all messages, continue
                                break 'queue;
                            }
                            Err(TryRecvError::Disconnected) => {
                                crate::bprintln!("{}Agent{} '{}' channel closed, terminating",
                                    crate::constants::FORMAT_BOLD,
                                    crate::constants::FORMAT_RESET,
                                    self.name);
                                break 'main;
                            }
                        }
                    }
                }
            }

            // Check if we've been terminated
            if matches!(self.state, AgentState::Terminated) {
                crate::bprintln!(
                    "{}Agent{} processing was terminated.",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET
                );
                break 'main;
            }
        }

        crate::bprintln!(
            "🤖 {}Agent{} '{}' terminated",
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET,
            self.name
        );
    }

    /// Handle incoming messages and commands
    async fn handle_message(&mut self, msg: AgentMessage) {
        match msg {
            AgentMessage::UserInput(input) => {
                // Add message to conversation and start processing
                self.conversation
                    .push(Message::text("user", input.clone(), MessageInfo::User));
                self.set_state(AgentState::Processing);
                // Display user input with chevron and dark blue color
                crate::bprintln!(
                    "{}{}>{} {}{}{}",
                    crate::constants::FORMAT_BLUE,
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET,
                    crate::constants::FORMAT_BLUE,
                    input,
                    crate::constants::FORMAT_RESET
                );
            }
            AgentMessage::AgentInput { content, source_id, source_name } => {
                // Format the message to indicate it's from another agent
                let formatted_message = format!(
                    "<agent_message source=\"{}\" source_id=\"{}\">\n{}\n</agent_message>",
                    source_name,
                    source_id,
                    content
                );

                // Add message to conversation with special formatting to indicate agent source
                self.conversation
                    .push(Message::text("user", formatted_message.clone(), MessageInfo::User));
                self.set_state(AgentState::Processing);

                // Display agent input with special formatting
                crate::bprintln!(
                    "{}{}[From Agent: {}]{} {}{}{}",
                    crate::constants::FORMAT_GREEN,
                    crate::constants::FORMAT_BOLD,
                    source_name,
                    crate::constants::FORMAT_RESET,
                    crate::constants::FORMAT_GREEN,
                    content,
                    crate::constants::FORMAT_RESET
                );
            }
            AgentMessage::Command(cmd) => {
                self.handle_command(cmd).await;
            }
            AgentMessage::Terminate => {
                crate::bprintln!(
                    "🤖 {}Agent{} '{}' received terminate message",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET,
                    self.name
                );
                self.set_state(AgentState::Terminated);
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
            AgentCommand::SetThinkingBudget(budget) => {
                self.set_thinking_budget(budget);
                crate::bprintln!("Thinking budget set to {} tokens", budget);
            }
        }
    }

    /// Execute a shell command with streaming output and interruption capability
    async fn execute_streaming_shell(
        &mut self,
        args: &str,
        body: &str,
        interrupt_coordinator: &InterruptCoordinator,
    ) -> Result<MessageResult, Box<dyn std::error::Error + Send + Sync>> {
        // Update state to running tool
        self.set_state(AgentState::RunningTool {
            tool: "shell".to_string(),
            interruptible: true,
        });

        // Args already contain the command arguments (everything after "shell")
        let cmd_args = args.trim().to_string();

        // Create interrupt data for coordination
        let interrupt_data = Arc::new(Mutex::new(InterruptData::new()));

        // Create channel for high-priority interrupt signals
        let (interrupt_tx, mut interrupt_rx) = mpsc::channel(10);

        // Update coordinator to indicate shell is running and should receive priority interrupts
        interrupt_coordinator.set_shell_running(true, Some(interrupt_tx));

        // Execute shell command and get the output receiver
        let silent_mode = self.tool_executor.is_silent();
        let mut rx = match execute_shell(&cmd_args, &body, interrupt_data.clone(), silent_mode).await {
            Ok(rx) => rx,
            Err(e) => {
                // Make sure to clean up interrupt state if startup fails
                interrupt_coordinator.set_shell_running(false, None);
                self.set_state(AgentState::Processing);

                return Err(Box::<dyn std::error::Error + Send + Sync>::from(format!(
                    "Shell execution error: {}",
                    e
                )));
            }
        };

        // Add a timestamp for tracking performance
        let start_time = std::time::Instant::now();
        
        // Buffer to collect output for the conversation history
        let mut partial_output = String::new();

        // This will hold the final message after command completion
        let result_message;
        let mut success = true;

        // Flag to track if we're in the process of interrupting
        let mut interrupting = false;
        // Store the reason for interruption if provided
        let mut interruption_reason_str: Option<String> = None;

        // Track time between interruption checks (to prevent excessive API calls)
        let mut last_check_time = std::time::Instant::now();

        // Configure interruption check interval based on command type
        // Shorter for commands that produce a lot of output quickly
        let min_check_interval =
            if cmd_args.contains("grep") || cmd_args.contains("find") || cmd_args.contains("watch")
            {
                Duration::from_secs(5) // Check more frequently for verbose commands
            } else {
                Duration::from_secs(10) // Standard interval for most commands
            };

        // Track if we have a partial tool result in the conversation
        let mut has_partial_result = false;

        // Loop to receive output and check for interruption
        loop {
            tokio::select! {
                // Process shell output
                output = rx.recv() => {
                    match output {
                        Some(ShellOutput::Stdout(line)) => {
                            // Sanitize line by removing ANSI escape sequences
                            let sanitized_line = strip_ansi_sequences(&line);

                            // Add sanitized output to full output record
                            partial_output.push_str(&sanitized_line);
                            partial_output.push('\n');
                            
                            // Check for interruption based on time or output volume
                            let should_check_interrupt = !interrupting && last_check_time.elapsed() > min_check_interval;

                            if should_check_interrupt {
                                // Update last check time
                                last_check_time = std::time::Instant::now();

                                // Remove previous partial result if it exists
                                if has_partial_result {
                                    self.conversation.pop();
                                }

                                // Create partial tool result message WITHOUT the ending tag
                                // Shell tools reuse the same index for both partial outputs and final result
                                let partial_tool_result = format!(
                                    "{}\n{}",
                                    self.grammar.tool_result_start_tag(self.tool_invocation_counter), 
                                    partial_output
                                );

                                // Mark this point in conversation as a cache point
                                self.cache_here();

                                // Create the partial result message
                                let partial_message = Message::text(
                                    "user",
                                    partial_tool_result,
                                    MessageInfo::ToolResult {
                                        tool_name: "shell".to_string(),
                                        tool_index: Some(self.tool_invocation_counter),
                                    }
                                );

                                // Add partial result to conversation and update tool mapper
                                let msg_index = self.conversation.len();
                                self.conversation.push(partial_message);


                                has_partial_result = true;

                                // Calculate elapsed time since the command started
                                let elapsed_duration = start_time.elapsed();
                                
                                // Send interruption check using the partial tool result and elapsed time
                                if let Ok(interruption_check) = self.check_for_interruption(elapsed_duration).await {
                                    if interruption_check.interrupted {
                                        // Store the interruption reason if provided
                                        let reason = interruption_check.reason.unwrap_or_else(||
                                            "No specific reason provided".to_string()
                                        );

                                        // Log the interruption before moving the reason
                                        crate::bprintln!("{}{}{} requested: {}{}",
                                            crate::constants::FORMAT_BOLD,
                                            crate::constants::FORMAT_BLUE,
                                            "LLM interruption",
                                            reason,
                                            crate::constants::FORMAT_RESET);

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
                            // Sanitize line by removing ANSI escape sequences
                            let sanitized_line = strip_ansi_sequences(&line);

                            // Add sanitized output to full output record
                            partial_output.push_str(&sanitized_line);
                            partial_output.push('\n');
                        },
                        Some(ShellOutput::Complete(tool_result)) => {
                            // Command completed, store results
                            success = tool_result.success;
                            // Note: tool_result.agent_output will be used when determining the final result_message
                            // Clear interrupt_shell as the command is done
                            // Update coordinator to indicate shell is no longer running
                            interrupt_coordinator.set_shell_running(false, None);
                            break;
                        },
                        None => {
                            // Channel closed, command must be done
                            // Clear interrupt_shell as the command is done
                            // Update coordinator to indicate shell is no longer running
                            interrupt_coordinator.set_shell_running(false, None);
                            break;
                        }
                    }
                },

                // Check for high-priority interrupts from dedicated channel
                interrupt_signal = interrupt_rx.recv() => {
                    if let Some(signal) = interrupt_signal {
                        // Handle immediate interrupt from dedicated channel
                        let reason = signal.reason.unwrap_or_else(||
                            "High-priority interrupt received".to_string()
                        );

                        // Log the interrupt with bold dark blue formatting
                        crate::bprintln!("{}{}{} interrupted: {}{}",
                            crate::constants::FORMAT_BOLD,
                            crate::constants::FORMAT_BLUE,
                            "Shell command",
                            reason,
                            crate::constants::FORMAT_RESET);

                        let mut data = interrupt_data.lock().unwrap();
                        data.interrupt(reason.clone());

                        interrupting = true;
                        interruption_reason_str = Some(reason);
                    } else {
                        // Channel closed unexpectedly
                        crate::berror_println!("Shell interrupt channel closed unexpectedly");
                    }
                },

                // Periodic check for interruption flag
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    // Check if already interrupted
                    if interrupt_data.lock().unwrap().is_interrupted() && !interrupting {
                        // This would happen if the interrupt came from somewhere else
                        interrupting = true;

                        // Get the reason if available
                        let reason = {
                            let data = interrupt_data.lock().unwrap();
                            data.reason().cloned()
                        };

                        interruption_reason_str = reason.clone();
                        crate::bprintln!("{}{}{} detected: {}{}",
                            crate::constants::FORMAT_BOLD,
                            crate::constants::FORMAT_BLUE,
                            "Shell interrupt",
                            reason.unwrap_or_else(|| "Unknown reason".to_string()),
                            crate::constants::FORMAT_RESET);
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
            // When interrupted by LLM or user, this is NOT an error, it's a successful interruption
            success = true;
        } else {
            result_message = format!("{}\n\n[COMMAND COMPLETED SUCCESSFULLY]", partial_output);
        }

        // Format the shell output with appropriate delimiters
        // Note: Interruption is NOT an error, so we use TOOL_RESULT for it
        let agent_response = if success || interrupting {
            self.grammar.format_tool_result(self.tool_invocation_counter, &result_message)
        } else {
            self.grammar.format_tool_error(self.tool_invocation_counter, &result_message)
        };

        // Add the agent_response to the conversation history
        // Interruption should be treated as a successful result
        let message_info = if success || interrupting {
            MessageInfo::ToolResult {
                tool_name: "shell".to_string(),
                tool_index: Some(self.tool_invocation_counter),
            }
        } else {
            MessageInfo::ToolError {
                tool_name: "shell".to_string(),
                tool_index: Some(self.tool_invocation_counter),
            }
        };

        // Create the message
        let message = Message::text("user", agent_response.clone(), message_info);

        // Add to conversation and update tool mapper
        let msg_index = self.conversation.len();
        self.conversation.push(message);

        // Reset state to Processing since we're continuing processing
        self.set_state(AgentState::Processing);

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
        elapsed_duration: Duration,
    ) -> Result<InterruptionCheck, Box<dyn std::error::Error + Send + Sync>> {
        // Save current cache points for efficient token usage
        let current_cache_points = self.cache_points.clone();

        // Format the elapsed time in a human-readable format
        let elapsed_seconds = elapsed_duration.as_secs();
        let elapsed_time_str = if elapsed_seconds < 60 {
            format!("{} seconds", elapsed_seconds)
        } else if elapsed_seconds < 3600 {
            format!("{} minutes {} seconds", elapsed_seconds / 60, elapsed_seconds % 60)
        } else {
            format!("{} hours {} minutes", 
                elapsed_seconds / 3600, 
                (elapsed_seconds % 3600) / 60
            )
        };

        // Create a tailored prompt for the interruption check
        let interruption_check_message = format!(
            "========== COMMAND INTERRUPTION CHECK ==========\n\
            The command has been running for {}.\n\
            Evaluate if this command should be interrupted based on its current output.\n\
            \n\
            Interrupt if:\n\
            - You have enough information to proceed\n\
            - The output is repetitive or redundant\n\
            - Errors indicate the command won't recover\n\
            - The command is producing excessive output with limited value\n\
            \n\
            RESPONSE FORMAT:\n\
            - To continue: '<continue/>'\n\
            - To interrupt: '<interrupt>ONE SENTENCE REASON</interrupt>'\n\
            \n\
            If interrupting, provide exactly ONE SENTENCE explaining why.\n\
            Your decision:"
            , elapsed_time_str
        );

        // Log interruption check with blue formatting
        crate::bprintln!(
            "{}{}Checking if shell command should be interrupted...{}",
            crate::constants::FORMAT_BLUE,
            crate::constants::FORMAT_BOLD,
            crate::constants::FORMAT_RESET
        );

        // Create a temporary message to add to conversation
        let temp_message = Message::text("user", interruption_check_message, MessageInfo::User);

        // Add message temporarily
        self.conversation.push(temp_message);

        // Use "</interrupt>" as stop sequence to allow content between tags
        let stop_sequences = vec!["</interrupt>".to_string(), "<continue/>".to_string()];

        // Allow 100 tokens for interruption reason (limited to keep costs low)
        let max_tokens_for_check = 100;

        // Start a timeout for the interruption check
        let timeout_duration = tokio::time::Duration::from_secs(10);

        // Handle the LLM response with proper error conversion and timeout
        let response = match tokio::time::timeout(
            timeout_duration,
            self.llm.send_message(
                &self.conversation,
                self.config.system_prompt.as_deref(), // Use the existing system prompt
                Some(&stop_sequences),
                None,
                Some(&current_cache_points),
                Some(max_tokens_for_check),
            ),
        )
        .await
        {
            Ok(result) => match result {
                Ok(response) => response,
                Err(e) => {
                    // Convert the error to a Send + Sync error by using the string representation
                    crate::berror_println!("Interruption check failed: {}", e);

                    // Remove the temporary message before returning
                    self.conversation.pop();

                    return Err(format!("Interruption check failed: {}", e).into());
                }
            },
            Err(_) => {
                // Timeout occurred - clean up and return no interruption
                crate::berror_println!(
                    "Interruption check timed out after {} seconds",
                    timeout_duration.as_secs()
                );

                // Remove the temporary message before returning
                self.conversation.pop();

                return Ok(InterruptionCheck {
                    interrupted: false,
                    reason: None,
                });
            }
        };

        // Remove the temporary message
        self.conversation.pop();

        // Check if we got a proper stop sequence
        if response.stop_reason.as_deref() != Some("stop_sequence") {
            crate::bprintln!("Interruption check completed: continue execution");
            return Ok(InterruptionCheck {
                interrupted: false,
                reason: None,
            });
        }

        // Extract and process the stop sequence
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

        // Log the decision
        if should_interrupt {
            crate::bprintln!(
                "{}{}{} requested: {}{}",
                crate::constants::FORMAT_BOLD,
                crate::constants::FORMAT_BLUE,
                "LLM interruption",
                reason,
                crate::constants::FORMAT_RESET
            );
        } else {
            crate::bprintln!(
                "{}{}{} to continue execution{}",
                crate::constants::FORMAT_BLUE,
                crate::constants::FORMAT_BOLD,
                "LLM decided",
                crate::constants::FORMAT_RESET
            );
        }

        Ok(InterruptionCheck {
            interrupted: should_interrupt,
            reason: if should_interrupt { Some(reason) } else { None },
        })
    }

    /// Load project information from the specified file if it exists and the conversation is empty
    ///
    /// # Arguments
    /// * `filepath` - Optional path to the project info file, defaults to ".autoswe" if None
    /// * `force` - If true, will add project info even if conversation is not empty
    ///
    /// # Returns
    /// * `Ok(true)` if project info was loaded
    /// * `Ok(false)` if no project info was loaded (file doesn't exist or not needed)
    /// * `Err(...)` if an error occurred while loading the file
    async fn load_project_info(
        &mut self,
        filepath: Option<&str>,
        force: bool,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let path = filepath.unwrap_or(".autoswe");

        // Check if we should add project info (either conversation is empty or force=true)
        // and the specified file exists
        if (self.conversation.is_empty() || force) && tokio::fs::try_exists(path).await? {
            let working = std::env::current_dir()?;
            let project_info = tokio::fs::read_to_string(path).await?;

            let content = format!(
                "# You're currently working in this directory:\n```\n{}\n```\n# Project information:\n{}",
                working.to_str().unwrap_or("unknown"),
                project_info
            );

            // Insert as a regular user message at the beginning
            self.conversation
                .push(Message::text("user", content, MessageInfo::User));

            crate::bprintln!(
                "{}Loaded project information from {} file{}",
                crate::constants::FORMAT_CYAN,
                path,
                crate::constants::FORMAT_RESET
            );

            return Ok(true);
        }

        Ok(false)
    }

    /// Send a message to the LLM backend and process the response
    pub async fn send_message(
        &mut self,
        interrupt_coordinator: &InterruptCoordinator,
    ) -> Result<MessageResult, Box<dyn std::error::Error + Send + Sync>> {
        // Load project information from .autoswe file if needed
        // Using default path (.autoswe) and only loading if conversation is empty
        self.load_project_info(None, false).await?;

        // Get necessary values for token counting
        let thinking_budget = Some(self.config.thinking_budget);
        let safe_token_limit = self.llm.safe_input_token_limit();

        // Apply conversation truncation if needed to stay within token limits
        // This prevents the conversation from exceeding the model's context window
        // by intelligently replacing older tool outputs with placeholders
        let needs_cache_reset = {
            // Temporary scope to limit borrow of system_prompt
            let system_prompt = self.config.system_prompt.as_deref();

            // Count tokens in the current conversation using the LLM backend's accurate counter
            // This ensures we have precise token counts for making truncation decisions
            match self
                .llm
                .count_tokens(&self.conversation, system_prompt)
                .await
            {
                Ok(usage) => {
                    // Only apply truncation if we successfully counted tokens
                    // Check if truncation is needed and apply it
                    if let Some(truncation_result) = truncate_conversation(
                        &mut self.conversation,
                        safe_token_limit,
                        &usage,
                        &self.truncation_config,
                    ) {
                        // Log truncation occurred
                        crate::bprintln!(
                            "🔍 {}Truncated{} {} tool outputs to save approximately {} tokens",
                            crate::constants::FORMAT_BOLD,
                            crate::constants::FORMAT_RESET,
                            truncation_result.truncated_messages,
                            truncation_result.estimated_tokens_saved
                        );

                        // Need to reset cache points after truncation
                        true
                    } else {
                        false
                    }
                }
                Err(e) => {
                    // Log token counting error but continue without truncation
                    crate::berror_println!("Failed to count tokens for truncation: {}", e);
                    false
                }
            }
        };

        // Reset cache points if needed
        if needs_cache_reset {
            self.reset_cache_points();
        }

        // Apply conversation maintenance to remove empty messages
        // This ensures the conversation structure is clean before sending to the LLM
        let removed_messages = sanitize_conversation(&mut self.conversation);
        if removed_messages > 0 {
            // Log that messages were removed
            crate::bprintln!(
                "🧹 {}Removed{} {} empty message(s) from conversation",
                crate::constants::FORMAT_BOLD,
                crate::constants::FORMAT_RESET,
                removed_messages
            );

            // Reset cache points since message structure changed
            self.reset_cache_points();
        }

        // Get the system prompt after any modifications to conversation
        let system_prompt = self.config.system_prompt.as_deref();

        // Handle the LLM response with proper error conversion
        let response = match self
            .llm
            .send_message(
                &self.conversation,
                system_prompt,
                self.stop_sequences.as_deref(),
                thinking_budget,
                Some(&self.cache_points),
                None, // Use default max_tokens
            )
            .await
        {
            Ok(response) => response,
            Err(e) => {
                // Convert the error to a Send + Sync error by using the string representation
                return Err(format!("LLM request failed: {}", e).into());
            }
        };

        // Extract content from response
        let mut assistant_message = String::new();
        for content in &response.content {
            if let Content::Text { text } = content {
                assistant_message.push_str(text);
            }
        }

        // Parse the assistant's response using this agent's grammar
        let parsed = self.grammar.parse_response(&assistant_message);

        // If tools are not enabled, or no tool was found, handle as a regular response
        if !self.config.enable_tools || parsed.tool.is_none() {
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

            return Ok(MessageResult {
                response: assistant_message,
                continue_processing: false, // Stop processing, wait for user input
                token_usage: response.usage,
            });
        }

        // At this point, we know we have a tool invocation
        let tool = parsed.tool.unwrap();
        let tool_name = tool.name;
        let tool_body = tool.body;

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
        if !parsed.prefix.is_empty() {
            crate::conversation::print_assistant_response(&parsed.prefix);
        }

        // Everything before and including the tool invocation (we need this for conversation history)
        let full_assistant_message = assistant_message.clone();

        // Create the tool call message
        let tool_call_message = Message::text(
            "assistant",
            full_assistant_message,
            MessageInfo::ToolCall {
                tool_name: tool_name.clone(),
                tool_index: Some(self.tool_invocation_counter),
            },
        );

        self.conversation.push(tool_call_message);

        // Increment the tool invocation counter for all tools
        self.tool_invocation_counter += 1;

        // Special handling for shell tool to support streaming and interruption
        if tool_name == "shell" {
            // Convert the parsed args to a space-separated string
            let tool_args = tool.args.join(" ");
            
            // Use a new dedicated interrupt channel
            let shell_result = self
                .execute_streaming_shell(&tool_args, &tool_body, &interrupt_coordinator)
                .await?;
            return Ok(shell_result);
        }

        // For other tools, update state
        let interruptible = false; // Only shell is interruptible for now
        self.state = AgentState::RunningTool {
            tool: tool_name.clone(),
            interruptible,
        };

        // Increment the tool invocation counter
        self.tool_invocation_counter += 1;

        // Execute the tool with pre-parsed components from grammar
        let tool_args = tool.args.join(" ");  // Join the args array into a string
        let tool_result = self.tool_executor.execute_with_parts(&tool_name, &tool_args, &tool_body).await;

        // Set the state back to Processing by default - will be updated by the tool's state_change if needed
        self.state = AgentState::Processing;

        // Format the agent response with appropriate delimiters
        let agent_response = if tool_result.success {
            self.grammar.format_tool_result(self.tool_invocation_counter, &tool_result.agent_output)
        } else {
            self.grammar.format_tool_error(self.tool_invocation_counter, &tool_result.agent_output)
        };

        // Return value to use in the process flow
        let result_for_response = tool_result.agent_output.clone();

        // Add the agent_response to the conversation history (for the LLM to see)
        // Determine the MessageInfo based on whether it was a successful tool execution
        let message_info = if tool_result.success {
            MessageInfo::ToolResult {
                tool_name: tool_name.clone(),
                tool_index: Some(self.tool_invocation_counter),
            }
        } else {
            MessageInfo::ToolError {
                tool_name: tool_name.clone(),
                tool_index: Some(self.tool_invocation_counter),
            }
        };

        let response_message_len = agent_response.len();

        let message = Message::text("user", agent_response.clone(), message_info);

        // Add to conversation and update tool mapper
        let msg_index = self.conversation.len();
        self.conversation.push(message);

        if response_message_len > 500 {
            self.cache_here();
        }

        // Handle state changes based on tool result
        match tool_result.state_change {
            crate::tools::AgentStateChange::Wait => {
                // Update state to Idle to wait for messages
                self.state = AgentState::Idle;
                crate::bprintln!(
                    "⏸️ {}Agent{} is now waiting for messages.",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET
                );
    
                return Ok(MessageResult {
                    response: result_for_response,
                    continue_processing: false,
                    token_usage: response.usage,
                });
            },
            crate::tools::AgentStateChange::Done => {
                // Update state to Done with the final response
                self.state = AgentState::Done(Some(result_for_response.clone()));
                crate::bprintln!(
                    "✅ {}Agent{} has marked task as completed.",
                    crate::constants::FORMAT_BOLD,
                    crate::constants::FORMAT_RESET
                );
    
                return Ok(MessageResult {
                    response: result_for_response,
                    continue_processing: false,
                    token_usage: response.usage,
                });
            },
            crate::tools::AgentStateChange::Continue => {
                // Continue normal processing, handled below
            }
        }

        // Return with continue_processing flag set to true to indicate tool processing should continue
        // The agent run loop will handle sending the next empty message
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
        // Reset the tool mapper
        // Reset state to Idle if it was Done
        if matches!(self.state, AgentState::Done(_)) {
            self.state = AgentState::Idle;
            crate::bprintln!(
                "🤖 {}Agent{} state reset to Idle.",
                crate::constants::FORMAT_BOLD,
                crate::constants::FORMAT_RESET
            );
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

    /// Load project information from a specified file
    ///
    /// # Arguments
    /// * `filepath` - Path to the project info file (optional, defaults to ".autoswe")
    /// * `force` - If true, will add project info even if conversation is not empty
    ///
    /// This method is useful for CLI interfaces or other external code that wants
    /// to explicitly load a project file.
    pub async fn load_project_file(
        &mut self,
        filepath: Option<&str>,
        force: bool,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.load_project_info(filepath, force).await
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
    
    /// Set the grammar implementation
    pub fn set_grammar(&mut self, grammar: Box<dyn Grammar>) {
        self.grammar = grammar;
        
        // Update stop sequences based on new grammar
        self.stop_sequences = Some(vec![
            self.grammar.stop_sequences().done_stop_sequence.to_string(),
            self.grammar.stop_sequences().error_stop_sequence.to_string(),
        ]);
    }

}
