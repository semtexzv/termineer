//! Grammar trait for prompt generation and output parsing
//!
//! This module defines a trait for handling the basic syntax elements
//! used in prompt generation, such as tag delimiters and markers, and
//! for parsing structured information from agent output.

use crate::constants::{
    TOOL_ERROR_START_PREFIX,
    PATCH_DELIMITER_AFTER, PATCH_DELIMITER_BEFORE, PATCH_DELIMITER_END, TOOL_END, TOOL_ERROR_END,
    TOOL_ERROR_START, TOOL_RESULT_END, TOOL_RESULT_START, TOOL_RESULT_START_PREFIX, TOOL_START,
};

/// Represents a tool invocation with name, arguments, and body content
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    pub name: String,
    pub args: Vec<String>,
    pub body: String,
}

/// Represents a step in the conversation
#[derive(Debug, Clone)]
pub struct ParsedResponse {
    // Agent prefix
    pub prefix: String,
    // Tool invocation
    pub tool: Option<ToolInvocation>,
}

/// Stop sequences for LLM generation
#[derive(Debug, Clone)]
pub struct StopSequences {
    pub done_stop_sequence: &'static str,
    pub error_stop_sequence: &'static str,
}

/// Grammar trait for prompt generation and parsing
pub trait Grammar: Send + Sync + 'static {
    /// Returns the stop sequences for this grammar
    fn stop_sequences(&self) -> StopSequences;
    
    /// Formats an error message with the given tool name, index, and content
    fn format_tool_error(&self, tool_name: &str, index: usize, content: &str) -> String;
    
    /// Formats a tool result message with the given tool name, index, and content
    fn format_tool_result(&self, tool_name: &str, index: usize, content: &str) -> String;
    
    /// Formats a tool call with the given name and content
    fn format_tool_call(&self, name: &str, content: &str) -> String;

    fn format_patch(&self, before: &str, after: &str) -> String {
        format!(
            "{}\n{}{}\n{}{}",
            PATCH_DELIMITER_BEFORE,
            before,
            PATCH_DELIMITER_AFTER,
            after,
            PATCH_DELIMITER_END,
        )
    }
    
    /// Parses a response from the assistant
    fn parse_response(&self, response: &str) -> ParsedResponse;
    
    /// Returns the tool start tag for this grammar
    fn tool_start_tag(&self) -> &str;
    
    /// Returns the tool end tag for this grammar
    fn tool_end_tag(&self) -> &str;
    
    /// Returns the tool result start tag for this grammar with the given index
    fn tool_result_start_tag(&self, index: usize) -> String;
    
    /// Returns the tool result end tag for this grammar
    fn tool_result_end_tag(&self) -> &str;
    
    /// Returns the tool error start tag for this grammar with the given index
    fn tool_error_start_tag(&self, index: usize) -> String;
    
    /// Returns the tool error end tag for this grammar
    fn tool_error_end_tag(&self) -> &str;
}

/// Old grammar implementation using traditional XML tags
#[derive(Debug, Clone)]
pub struct OldGrammar;

impl Grammar for OldGrammar {
    fn stop_sequences(&self) -> StopSequences {
        StopSequences {
            done_stop_sequence: TOOL_RESULT_START,
            error_stop_sequence: TOOL_RESULT_END,
        }
    }
    
    fn format_tool_error(&self, tool_name: &str, index: usize, content: &str) -> String {
        let tool_attribute = format!(" tool=\"{}\"", tool_name);
        let tag = format!("{} index=\"{}\"{}>", TOOL_ERROR_START_PREFIX, index, tool_attribute);
        let content = content.trim();
        let separator = if !content.contains('\n') {
            ""
        } else {
            "\n"
        };
        format!(
            "{}{}{}{}",
            tag,
            content,
            separator,
            TOOL_ERROR_END
        )
    }
    
    fn format_tool_result(&self, tool_name: &str, index: usize, content: &str) -> String {
        let tool_attribute = format!(" tool=\"{}\"", tool_name);
        let tag = format!("{} index=\"{}\"{}>", TOOL_RESULT_START_PREFIX, index, tool_attribute);
        let content = content.trim();
        let separator = if !content.contains('\n') {
            ""
        } else {
            "\n"
        };
        format!(
            "{}{}{}{}",
            tag,
            content,
            separator,
            TOOL_RESULT_END
        )
    }
    
    fn format_tool_call(&self, name: &str, content: &str) -> String {
        format!("{}{} {}{}", TOOL_START, name, content, TOOL_END)
    }
    
    fn parse_response(&self, response: &str) -> ParsedResponse {
        // Direct implementation of parsing logic to avoid circular dependency
        if !response.contains(TOOL_START) {
            // No tool invocation found
            return ParsedResponse {
                prefix: response.to_string(),
                tool: None,
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

                let (header, body) = match tool_content.trim().split_once('\n') {
                    Some((header, body)) => (header, body),
                    None => (tool_content, ""),
                };

                let mut args = header.split_whitespace().collect::<Vec<&str>>();

                if args.is_empty() {
                    // No tool name found, return the original text
                    return ParsedResponse {
                        prefix: response.to_string(),
                        tool: None,
                    };
                }

                let tool_name = args.remove(0).to_lowercase();

                return ParsedResponse {
                    prefix: text_before_tool,
                    tool: Some(ToolInvocation {
                        name: tool_name,
                        args: args.iter().map(|s| s.to_string()).collect::<Vec<String>>(),
                        body: body.to_string(),
                    }),
                };
            }
        }
        
        // If we reach here, no proper tool invocation was found
        ParsedResponse {
            prefix: response.to_string(),
            tool: None,
        }
    }
    
    fn tool_start_tag(&self) -> &str {
        TOOL_START
    }
    
    fn tool_end_tag(&self) -> &str {
        TOOL_END
    }
    
    fn tool_result_start_tag(&self, index: usize) -> String {
        format!("{} index=\"{}\">", TOOL_RESULT_START_PREFIX, index)
    }
    
    fn tool_result_end_tag(&self) -> &str {
        TOOL_RESULT_END
    }
    
    fn tool_error_start_tag(&self, index: usize) -> String {
        format!("{} index=\"{}\">", TOOL_ERROR_START_PREFIX, index)
    }
    
    fn tool_error_end_tag(&self) -> &str {
        TOOL_ERROR_END
    }
}
