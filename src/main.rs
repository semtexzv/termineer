//! autoswe console interface for Claude API
//!
//! A command-line interface for interacting with Claude AI through Anthropic's API.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io::{self, Write};

mod commands;
mod config;
mod constants;
pub mod jsonpath;
mod llm;
mod prompts;
pub mod serde_element_array;
mod session;
mod tools;

use config::{ArgResult, Config};
use constants::{TOOL_ERROR_START, TOOL_RESULT_START};
use llm::anthropic::Anthropic;
use llm::{Backend, Content, Message, MessageInfo, TokenUsage};
use prompts::{generate_minimal_system_prompt, ToolDocOptions};
use tools::ToolExecutor;

/// Result of sending a message, including whether further processing is needed
struct MessageResult {
    response: String,
    continue_processing: bool,
    token_usage: Option<TokenUsage>,
}

/// Client for interacting with Claude API
pub struct ClaudeClient {
    pub config: Config,
    llm: Box<dyn Backend>,
    tool_executor: ToolExecutor,
    pub conversation: Vec<Message>,
    pub readonly_mode: bool,
    pub stop_sequences: Option<Vec<String>>,
    pub cache_points: BTreeSet<usize>,
}

impl ClaudeClient {
    fn new(config: Config) -> Self {
        // Create LLM backend based on config
        let llm: Box<dyn llm::Backend> = Box::new(Anthropic::new(
            config.api_key.clone(),
            config.model.clone(),
        ));

        let tool_executor = ToolExecutor::new(false);

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
    fn clear_conversation(&mut self) {
        self.conversation.clear();
        // Clear all cache points when conversation is cleared
        self.cache_points.clear();
    }

    /// Set the system prompt
    fn set_system_prompt(&mut self, prompt: String) {
        self.config.system_prompt = Some(prompt);
        // System prompt change invalidates cache
        self.reset_cache_points();
    }

    /// Enable or disable tools
    fn enable_tools(&mut self, enabled: bool) {
        self.config.enable_tools = enabled;
    }

    /// Set the thinking budget
    fn set_thinking_budget(&mut self, budget: usize) {
        self.config.thinking_budget = budget;
    }

    /// Set the model to use
    fn set_model(&mut self, model: String) {
        self.config.model = model.clone();
        // Create new LLM provider with updated model
        self.llm = Box::new(Anthropic::new(self.config.api_key.clone(), model));
        // Reset cache points since model changed
        self.reset_cache_points();
    }

    /// Create a subagent configuration with read-only tools
    /// Returns a new ClaudeClient instance configured for subagent use
    pub fn create_subagent_for_task(task_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Get the API key from the environment
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "ANTHROPIC_API_KEY environment variable not set")?;

        // Create a new config for the subagent
        let mut subagent_config = crate::Config::new(api_key);

        // Use minimal prompt to save tokens
        let options = ToolDocOptions::readonly();
        let minimal_prompt = generate_minimal_system_prompt(&options);

        // Create task-specific system prompt
        let subagent_system_prompt = format!(
            "{}\n\nYou are a subagent created to complete a specific task.\n\
            Your task is: \"{}\"\n\
            Complete this task thoroughly and return your results using the done tool when finished.\n\
            You must put your final results in the done tool.\n\
            Note: You are in read-only mode and can only use shell, read, fetch, and done tools.", 
            minimal_prompt, task_name
        );

        subagent_config.system_prompt = Some(subagent_system_prompt);

        // Create new client
        let mut subagent = Self::new(subagent_config);
        subagent.enable_tools(true);
        subagent.readonly_mode = true;

        Ok(subagent)
    }

    /// Send a message to Claude and process the response
    fn send_message(
        &mut self,
        user_message: &str,
    ) -> Result<MessageResult, Box<dyn std::error::Error>> {
        // Import constants locally to avoid cluttering the global namespace
        use constants::{
            TOOL_END, TOOL_ERROR_END, TOOL_ERROR_START, TOOL_RESULT_END, TOOL_RESULT_START,
            TOOL_START,
        };

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

        // Update cache points if needed
        if self.cache_points.is_empty()
            || *self.cache_points.iter().rev().next().unwrap()
                < self.conversation.len().saturating_sub(5)
        {
            self.cache_points.insert(self.conversation.len() - 1);
        }

        if self.cache_points.len() > 5 {
            let point = *self.cache_points.iter().next().unwrap();
            self.cache_points.remove(&point);
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

        // This will be the final response to return
        let final_response;

        // Special handling for responses with tool invocations
        if self.config.enable_tools && assistant_message.contains(TOOL_START) {
            // Find the complete tool invocation (from start to end tag)
            if let Some(tool_start_idx) = assistant_message.find(TOOL_START) {
                if let Some(tool_end_relative_idx) =
                    assistant_message[tool_start_idx..].find(TOOL_END)
                {
                    // Complete end position (including the end tag)
                    let tool_end_idx = tool_start_idx + tool_end_relative_idx;
                    let complete_end_idx = tool_end_idx + TOOL_END.len();

                    // Everything before and including the tool invocation
                    let assistant_part = assistant_message[0..complete_end_idx].to_string();

                    // Process the tool to get the result
                    let tool_content =
                        &assistant_message[tool_start_idx + TOOL_START.len()..tool_end_idx];

                    // Extract the tool name for checking if it's the "done" tool
                    let parts: Vec<&str> =
                        tool_content.trim().splitn(2, char::is_whitespace).collect();
                    let tool_name = if !parts.is_empty() {
                        parts[0].to_lowercase()
                    } else {
                        "unknown".to_string()
                    };
                    let is_done_tool = !parts.is_empty() && tool_name == "done";

                    // Extract Claude's text part before executing the tool
                    let assistant_message_content =
                        if let Some(tool_start_pos) = assistant_part.find(TOOL_START) {
                            assistant_part[0..tool_start_pos].trim().to_string()
                        } else {
                            assistant_part.clone()
                        };

                    // Display Claude's text before executing the tool
                    if !assistant_message_content.is_empty() {
                        println!("Claude: {}", assistant_message_content);
                    }

                    // Execute the tool using our tool executor
                    let tool_result = self.tool_executor.execute(tool_content);

                    // Format the tool result as a user message
                    // User sees the user_output without delimiters, but Claude (agent) gets the agent_output with delimiters
                    let user_response = if tool_result.success {
                        tool_result.user_output.clone()
                    } else {
                        format!("[ERROR] {}", tool_result.user_output)
                    };

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

                    // Add the assistant's response (with tool invocation) to conversation history
                    self.conversation.push(Message::text(
                        "assistant",
                        assistant_part.clone(),
                        MessageInfo::ToolCall {
                            tool_name: tool_name.clone(),
                        },
                    ));

                    // Add the agent_response to the conversation history (for Claude to see)
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

                    // For the final response, just use the tool response since we already
                    // displayed Claude's text part before executing the tool
                    final_response = user_response;

                    // If this was the "done" tool, return the final response with continue_processing=false
                    if is_done_tool {
                        return Ok(MessageResult {
                            response: final_response,
                            continue_processing: false,
                            token_usage: response.usage,
                        });
                    }
                } else {
                    // Tool start tag found but no end tag - throw an error
                    return Err(
                        "Incomplete tool invocation: Found tool start tag but no matching end tag"
                            .into(),
                    );
                }
            } else {
                // No tool invocation found
                self.conversation.push(Message::text(
                    "assistant",
                    assistant_message.clone(),
                    MessageInfo::Assistant,
                ));

                final_response = assistant_message.clone();

                // No tool invocation, so don't continue processing
                return Ok(MessageResult {
                    response: final_response,
                    continue_processing: false,
                    token_usage: response.usage,
                });
            }
        } else {
            // No tools enabled or no tool markers in response
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

        // Return with continue_processing flag set to true to indicate tool processing
        Ok(MessageResult {
            response: final_response,
            continue_processing: true,
            token_usage: response.usage,
        })
    }
}

/// Read a line of input from the user
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

/// Process a user query, handling tool calls and multi-turn interactions
///
/// When silent_mode is true, no console output is produced (for subagents)
/// Returns a tuple of (task_completed, last_response)
fn process_user_query(
    client: &mut ClaudeClient,
    user_input: &str,
    silent_mode: bool,
) -> Result<(bool, String), Box<dyn std::error::Error>> {
    let mut message_sent = false;

    // Loop until we get a "done" tool or no further processing is needed
    loop {
        // Send the message to Claude
        let message_result = if !message_sent {
            // First message is the user's input
            message_sent = true;
            client.send_message(user_input)
        } else {
            // Subsequent messages are empty - Claude will continue with tool output
            client.send_message("")
        };

        match message_result {
            Ok(result) => {
                // Get the current response to return
                let final_response = result.response.clone();

                // Only print output to console if not in silent mode
                if !silent_mode {
                    // Display token usage statistics if available
                    if let Some(usage) = &result.token_usage {
                        use constants::{FORMAT_GRAY, FORMAT_RESET};

                        // Show minimal token usage for the current request only
                        println!(
                            "Claude: {}[{} in / {} out] ({} read, {} written){}",
                            FORMAT_GRAY,
                            usage.input_tokens,
                            usage.output_tokens,
                            usage.cache_read_input_tokens,
                            usage.cache_creation_input_tokens,
                            FORMAT_RESET
                        );
                    } else {
                        println!("\nClaude: ");
                    }

                    // Small delay to prevent tight loop, only when outputting to console
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }

                // Check for task completion by looking for the done tool
                let task_completed = client.conversation.last().map(|m|
                    matches!(&m.info,
                        MessageInfo::ToolResult { tool_name } | MessageInfo::ToolError { tool_name } if tool_name == "done"
                    )
                ).unwrap_or(false);

                // Check if we should continue processing
                if !result.continue_processing || task_completed {
                    return Ok((task_completed, final_response));
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

/// Run in interactive mode with a conversation UI
fn run_interactive_mode(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ClaudeClient::new(config.clone());

    // Apply appropriate system prompt if none is provided
    if client.config.system_prompt.is_none() {
        // Use default (full) tool options
        let options = ToolDocOptions::default();

        if client.config.use_minimal_prompt {
            // Use minimal prompt if configured
            let minimal_prompt = generate_minimal_system_prompt(&options);
            client.set_system_prompt(minimal_prompt);
        } else {
            // Use full default system prompt
            let default_prompt = prompts::generate_system_prompt(&options);
            client.set_system_prompt(default_prompt);
        }
    }

    // Attempt to resume last session if requested
    if config.resume_last_session {
        match session::load_last_session(&mut client) {
            Ok(_) => println!("Successfully resumed last session"),
            Err(e) => println!("Note: Could not resume last session: {}", e),
        }
    }

    println!("Claude Console Interface");
    println!("Type your message and press Enter to chat with Claude");
    println!("Type '/help' for available commands or '/exit' to quit");

    if client.config.enable_tools {
        println!("Tools are ENABLED. Claude will use tools automatically based on your request.");
    } else {
        println!("Tools are DISABLED. Use /tools on to enable them.");
    }

    // Display token optimization settings
    println!("\nToken optimization settings:");
    println!(
        "- Thinking budget: {} tokens",
        client.config.thinking_budget
    );
    println!(
        "- System prompt: {}",
        if client.config.system_prompt.is_some() {
            "custom"
        } else if client.config.use_minimal_prompt {
            "minimal"
        } else {
            "standard"
        }
    );
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

        // Handle exit command directly for quick exit
        if user_input == "/exit" {
            break;
        }

        // Handle commands
        if user_input.starts_with('/') {
            if let Err(e) = commands::handle_command(&mut client, user_input) {
                println!("Command error: {}", e);
            }
            continue;
        }

        // Process normal user query
        if !user_input.is_empty() {
            if let Err(e) = process_user_query(&mut client, user_input, false) {
                println!("\nError: {}\n", e);
            }
        }
    }

    println!("Goodbye!");
    Ok(())
}

/// Run a single query in non-interactive mode
fn run_query(config: Config, query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ClaudeClient::new(config.clone());

    // Apply appropriate system prompt if none is provided
    if client.config.system_prompt.is_none() {
        // Use default (full) tool options
        let options = ToolDocOptions::default();

        if client.config.use_minimal_prompt {
            // Use minimal prompt if configured
            let minimal_prompt = generate_minimal_system_prompt(&options);
            client.set_system_prompt(minimal_prompt);
        } else {
            // Use full default system prompt
            let default_prompt = prompts::generate_system_prompt(&options);
            client.set_system_prompt(default_prompt);
        }
    }

    // Attempt to resume last session if requested
    if config.resume_last_session {
        match session::load_last_session(&mut client) {
            Ok(_) => println!("Successfully resumed last session"),
            Err(e) => println!("Note: Could not resume last session: {}", e),
        }
    }

    // Process the query
    let result = process_user_query(&mut client, query, false);

    // Only care about success/error, not the actual result values
    result.map(|_| ())
}

/// Display usage text
fn print_usage() {
    use constants::*;

    // Create usage text using template
    let usage_text = format_template(USAGE_TEMPLATE);

    eprintln!("{}", usage_text);
}

/// Main entry point
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if exists
    let _ = dotenvy::dotenv();

    // Load configuration from environment variables
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Please set ANTHROPIC_API_KEY in a .env file or as an environment variable");
            eprintln!("Example .env file content:");
            eprintln!("ANTHROPIC_API_KEY=your_api_key_here");
            return Ok(());
        }
    };

    // Apply command line arguments to override configuration
    let args: Vec<String> = env::args().collect();
    let mut config = config; // Make config mutable to apply args

    // Apply args and get result type (query, interactive, or help)
    let arg_result = config.apply_args(&args);

    // Handle the different result types
    match arg_result {
        Ok(ArgResult::Query(query)) => run_query(config, &query)?,
        Ok(ArgResult::Interactive) => run_interactive_mode(config)?,
        Ok(ArgResult::ShowHelp) => {
            print_usage();
        }
        Err(e) => {
            eprintln!("{}", e);
        }
    }

    Ok(())
}
