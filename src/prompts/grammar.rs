//! Grammar trait for prompt generation and output parsing
//!
//! This module defines a trait for handling the basic syntax elements
//! used in prompt generation, such as tag delimiters and markers, and
//! for parsing structured information from agent output.

use crate::bprintln;
use crate::constants::{
    TOOL_ERROR_START_PREFIX,
    PATCH_DELIMITER_AFTER, PATCH_DELIMITER_BEFORE, PATCH_DELIMITER_END, TOOL_END, TOOL_ERROR_END,
    TOOL_ERROR_START, TOOL_RESULT_END, TOOL_RESULT_START, TOOL_RESULT_START_PREFIX, TOOL_START,
};

// Constants for markdown-based grammar
const MD_TOOL_CALL_START: &str = "```tool_use ";
const MD_TOOL_RESULT_START: &str = "```result [";
const MD_TOOL_ERROR_START: &str = "```error [";
const MD_CODE_END: &str = "```";

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
/// Note, these won't be included in the response
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
pub struct XmlGrammar;

impl Grammar for XmlGrammar {
    fn stop_sequences(&self) -> StopSequences {
        StopSequences {
            done_stop_sequence: TOOL_RESULT_START_PREFIX,
            error_stop_sequence: TOOL_ERROR_START_PREFIX,
        }
    }
    
    fn format_tool_error(&self, tool_name: &str, index: usize, content: &str) -> String {
        let tool_attribute = format!(" tool=\"{}\"", tool_name);
        let tag = format!("{} index=\"{}\"{}>", TOOL_ERROR_START_PREFIX, index, tool_attribute);
        let separator = "\n";
        format!(
            "{}{}{}{}",
            tag,
            content.trim_end(),  // Use the original content with preserved newlines
            separator,
            TOOL_ERROR_END
        )
    }
    
    fn format_tool_result(&self, tool_name: &str, index: usize, content: &str) -> String {
        let tool_attribute = format!(" tool=\"{}\"", tool_name);
        let tag = format!("{} index=\"{}\"{}>", TOOL_RESULT_START_PREFIX, index, tool_attribute);
        let separator = "\n";
        format!(
            "{}{}{}{}",
            tag,
            content.trim_end(),
            separator,
            TOOL_RESULT_END
        )
    }
    
    fn format_tool_call(&self, name: &str, content: &str) -> String {
        let trimmed_content = content.trim();
        
        // If content is empty or just whitespace, don't add a space
        if trimmed_content.is_empty() {
            return format!("{}{}{}", TOOL_START, name, TOOL_END);
        }
        
        
        // For simple single-line content, just add a space
        format!("{}{} {}{}", TOOL_START, name, trimmed_content, TOOL_END)
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
                
                bprintln!("Tool header: {:?}\nTool body: {:?}\n", header, body);

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

/// Module for handling different grammar implementations
pub mod formats {
    use std::sync::Arc;
    use super::*;

    /// Enum for the available grammar types
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum GrammarType {
        /// Traditional XML-based tags
        XmlTags,
        
        /// Markdown code blocks
        MarkdownBlocks,
    }

    /// Get a grammar implementation by type
    pub fn get_grammar(grammar_type: GrammarType) -> Arc<dyn Grammar> {
        match grammar_type {
            GrammarType::XmlTags => Arc::new(XmlGrammar),
            GrammarType::MarkdownBlocks => Arc::new(MarkdownGrammar),
        }
    }

    /// Get the default grammar implementation
    pub fn default_grammar() -> Arc<dyn Grammar> {
        get_grammar(GrammarType::XmlTags)
    }
}

/// Markdown-based grammar using code blocks
///
/// This implements tools as:
/// ```tool_call <name> [args]
/// [body]
/// ```
///
/// Results as:
/// ```tool_result[index]
/// [result content]
/// ```
///
/// Errors as:
/// ```tool_error[index]
/// [error content]
/// ```
#[derive(Debug, Clone)]
pub struct MarkdownGrammar;

impl Grammar for MarkdownGrammar {
    fn stop_sequences(&self) -> StopSequences {
        StopSequences {
            done_stop_sequence: MD_TOOL_RESULT_START,
            error_stop_sequence: MD_TOOL_ERROR_START, // Reverting to original - this is correct
        }
    }
    
    fn format_tool_error(&self, tool_name: &str, index: usize, content: &str) -> String {
        // Format: ```tool_error[index]
        //         error content
        //         ```
        format!(
            "{}{}]\n{}\n{}",
            MD_TOOL_ERROR_START,
            index,
            content.trim_end(), // Trim trailing whitespace but preserve internal newlines
            MD_CODE_END
        )
    }
    
    fn format_tool_result(&self, tool_name: &str, index: usize, content: &str) -> String {
        // Format: ```tool_result[index]
        //         result content
        //         ```
        format!(
            "{}{}]\n{}\n{}",
            MD_TOOL_RESULT_START,
            index,
            content.trim_end(), // Trim trailing whitespace but preserve internal newlines
            MD_CODE_END
        )
    }
    
    fn format_tool_call(&self, name: &str, content: &str) -> String {
        // Format: ```tool_call <name> [args]
        //         [body content if any]
        //         ```
        let trimmed_content = content.trim();
        
        if trimmed_content.is_empty() {
            // No content, just the tool name
            return format!("{}{}\n{}", MD_TOOL_CALL_START, name, MD_CODE_END);
        }
        
        // If content has newlines, format accordingly
        if trimmed_content.contains('\n') {
            return format!("{}{}\n{}\n{}", 
                MD_TOOL_CALL_START, 
                name, 
                trimmed_content,
                MD_CODE_END
            );
        }
        
        // For simple single-line content
        format!("{}{} {}\n{}", MD_TOOL_CALL_START, name, trimmed_content, MD_CODE_END)
    }
    
    fn parse_response(&self, response: &str) -> ParsedResponse {
        bprintln!("Parsing markdown response: {}\n\n", response);
        // Defensive programming - return early if response is empty or too short
        if response.is_empty() || response.len() < MD_TOOL_CALL_START.len() + 3 {
            return ParsedResponse {
                prefix: response.to_string(),
                tool: None,
            };
        }
        
        // Check for markdown tool call pattern
        if !response.contains(MD_TOOL_CALL_START) {
            // No tool invocation found
            return ParsedResponse {
                prefix: response.to_string(),
                tool: None,
            };
        }

        // Find the tool invocation (from markdown start to end)
        if let Some(tool_start_idx) = response.find(MD_TOOL_CALL_START) {
            // Safely get the substring after the tool call start
            let after_tool_start = match response.get(tool_start_idx..) {
                Some(s) => s,
                None => {
                    // This shouldn't happen given our earlier check, but just in case
                    return ParsedResponse {
                        prefix: response.to_string(),
                        tool: None,
                    };
                }
            };
            
            if let Some(code_end_relative_idx) = after_tool_start.find(MD_CODE_END) {

                let code_end_idx = tool_start_idx + code_end_relative_idx;

                // Get the text before the tool invocation (safely)
                let text_before_tool = if tool_start_idx > 0 {
                    response[0..tool_start_idx].trim().to_string()
                } else {
                    String::new()
                };

                // Extract the entire code block content (without the backticks and marker)
                // Use safe substring operations with bounds checking
                let block_start_idx = tool_start_idx + MD_TOOL_CALL_START.len();
                let block_content = if block_start_idx < code_end_idx {
                    match response.get(block_start_idx..code_end_idx) {
                        Some(content) => content,
                        None => {
                            // Bounds error - fall back to treating as plain text
                            return ParsedResponse {
                                prefix: response.to_string(),
                                tool: None,
                            };
                        }
                    }
                } else {
                    // Empty block content
                    ""
                };

                // Split into first line (tool name + args) and the rest (body)
                // Use more robust approach to handle different line endings
                let trimmed_block = block_content.trim();
                let (header, body) = match trimmed_block.find('\n') {
                    Some(newline_pos) => {
                        let (h, b) = trimmed_block.split_at(newline_pos);
                        // Skip the newline character when returning body
                        (h, if b.len() > 1 { &b[1..] } else { "" })
                    },
                    None => (trimmed_block, ""),
                };

                // Parse the tool name and args from the header
                let mut args = header.trim().split_whitespace().collect::<Vec<&str>>();
                
                if args.is_empty() {
                    // No tool name found - return the original text
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
        MD_TOOL_CALL_START
    }
    
    fn tool_end_tag(&self) -> &str {
        MD_CODE_END
    }
    
    fn tool_result_start_tag(&self, index: usize) -> String {
        format!("{}{}]", MD_TOOL_RESULT_START, index)
    }
    
    fn tool_result_end_tag(&self) -> &str {
        MD_CODE_END
    }
    
    fn tool_error_start_tag(&self, index: usize) -> String {
        format!("{}{}]", MD_TOOL_ERROR_START, index)
    }
    
    fn tool_error_end_tag(&self) -> &str {
        MD_CODE_END
    }
}
