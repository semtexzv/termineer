//! Conversation parsing and formatting
//!
//! This module handles parsing of LLM responses and formatting for display.

use crate::constants::{FORMAT_GRAY, FORMAT_RESET, TOOL_END, TOOL_START};
use crate::llm::TokenUsage;

/// Print the assistant's response to the output buffer
pub fn print_assistant_response(text: &str) {
    crate::bprintln!("{}", text);
}

/// Print token usage statistics to the output buffer
pub fn print_token_stats(usage: &TokenUsage) {
    crate::bprintln!(
        "{}{}[{} in / {} out] ({} read, {} written){}",
        FORMAT_GRAY,
        crate::constants::FORMAT_BOLD,
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_read_input_tokens,
        usage.cache_creation_input_tokens,
        FORMAT_RESET
    );
}

/// Parsed response from the assistant
pub struct ParsedResponse {
    /// Text before any tool invocation
    pub text: String,
    /// Tool name if found
    pub tool_name: Option<String>,
    /// Tool content if found
    pub tool_content: Option<String>,
    /// Whether processing should continue (true if a tool was found)
    /// This field is used for structural clarity in the API but not directly accessed
    #[allow(dead_code)]
    pub continue_processing: bool,
}

/// Parse the assistant's response to extract text and tool invocations
pub fn parse_assistant_response(response: &str) -> ParsedResponse {
    if !response.contains(TOOL_START) {
        // No tool invocation found
        return ParsedResponse {
            text: response.to_string(),
            tool_name: None,
            tool_content: None,
            continue_processing: false,
        };
    }

    // Find the tool invocation (from start to end tag)
    if let Some(tool_start_idx) = response.find(TOOL_START) {
        if let Some(tool_end_relative_idx) = response[tool_start_idx..].find(TOOL_END) {
            let tool_end_idx = tool_start_idx + tool_end_relative_idx;

            // Get the text before the tool invocation
            let text_before_tool = response[0..tool_start_idx].trim().to_string();

            // Extract tool content
            let tool_content = &response[tool_start_idx + TOOL_START.len()..tool_end_idx];

            // Extract tool name
            let parts: Vec<&str> = tool_content.trim().splitn(2, char::is_whitespace).collect();
            let tool_name = if !parts.is_empty() {
                parts[0].to_lowercase()
            } else {
                "unknown".to_string()
            };

            // Return parsed response
            return ParsedResponse {
                text: text_before_tool,
                tool_name: Some(tool_name),
                tool_content: Some(tool_content.to_string()),
                continue_processing: true,
            };
        }
    }

    // Fallback if we couldn't properly parse the tool
    ParsedResponse {
        text: response.to_string(),
        tool_name: None,
        tool_content: None,
        continue_processing: false,
    }
}

/// Check if a tool name is the "done" tool
pub fn is_done_tool(tool_name: &str) -> bool {
    tool_name == "done"
}

// Function removed - we now use a direct string check for "AGENT_WAITING_STATE_ACTIVE"