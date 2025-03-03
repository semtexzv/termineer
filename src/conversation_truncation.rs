//! Conversation truncation service
//!
//! This module provides functionality to truncate conversations
//! when they approach token limits, replacing tool outputs with placeholders.

use crate::llm::{Message, MessageInfo, Content, TokenUsage};
use std::collections::BTreeSet;

/// Configuration for conversation truncation
pub struct TruncationConfig {
    /// Percentage of the safe input token limit to trigger truncation (e.g., 0.8 = 80%)
    pub threshold_percentage: f64,
    
    /// Number of initial tool outputs to preserve (often file listings)
    pub preserve_initial_tools: usize,
    
    /// Number of recent tool outputs to preserve
    pub preserve_recent_tools: usize,
    
    /// Maximum size (in chars) to preserve from large tool outputs when truncating
    pub max_preserved_chars: usize,
    
    /// Placeholder text to use for truncated tool outputs
    pub placeholder_text: String,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            threshold_percentage: 0.8,
            preserve_initial_tools: 3,
            preserve_recent_tools: 5,
            max_preserved_chars: 200,
            placeholder_text: "[Tool output truncated to save context space]".to_string(),
        }
    }
}

/// Result of truncation operation
pub struct TruncationResult {
    /// Number of messages that were truncated
    pub truncated_messages: usize,
    
    /// Estimated tokens saved (approximate)
    pub estimated_tokens_saved: usize,
    
    /// Indices of truncated messages
    pub truncated_indices: BTreeSet<usize>,
}

/// Tool result info used for truncation analysis
struct ToolResultInfo {
    /// Index of the message in the conversation
    index: usize,
    /// Name of the tool
    tool_name: String,
    /// Length of the content in characters
    content_length: usize,
}

/// Identifies and truncates eligible tool outputs in a conversation
///
/// # Arguments
/// * `messages` - The conversation messages to analyze and modify
/// * `safe_token_limit` - The safe token limit for the model
/// * `current_tokens` - Current token usage information
/// * `config` - Configuration for truncation behavior
///
/// # Returns
/// Information about the truncation performed, or None if no truncation was needed
pub fn truncate_conversation(
    messages: &mut Vec<Message>,
    safe_token_limit: usize,
    current_tokens: &TokenUsage,
    config: &TruncationConfig,
) -> Option<TruncationResult> {
    // Check if we need to truncate
    if !should_truncate(current_tokens, safe_token_limit, config) {
        return None;
    }
    
    // Find all tool result messages and their info
    let tool_results = collect_tool_results(messages);
    
    // If we don't have enough tool outputs to truncate, return None
    if tool_results.len() <= config.preserve_initial_tools + config.preserve_recent_tools {
        return None;
    }
    
    // Determine which indices to truncate
    let truncation_candidates = identify_truncation_candidates(&tool_results, config);
    
    // Apply truncation
    let result = apply_truncation(messages, &truncation_candidates, config);
    
    Some(result)
}

/// Determines if truncation is needed based on token count and model limit
///
/// # Arguments
/// * `token_usage` - Current token usage information
/// * `safe_token_limit` - Safe token limit for the current model
/// * * `config` - Truncation configuration
///
/// # Returns
/// `true` if truncation is needed, `false` otherwise
fn should_truncate(
    token_usage: &TokenUsage, 
    safe_token_limit: usize,
    config: &TruncationConfig,
) -> bool {
    let threshold = (safe_token_limit as f64 * config.threshold_percentage) as usize;
    token_usage.input_tokens >= threshold
}

/// Collect all tool result messages from the conversation
fn collect_tool_results(messages: &[Message]) -> Vec<ToolResultInfo> {
    let mut tool_results = Vec::new();
    
    for (i, message) in messages.iter().enumerate() {
        match &message.info {
            MessageInfo::ToolResult { tool_name, .. } => {
                // Skip "done" tool which is typically important
                if tool_name != "done" {
                    let content_length = match &message.content {
                        Content::Text { text } => text.len(),
                        _ => 0,
                    };
                    
                    tool_results.push(ToolResultInfo {
                        index: i,
                        tool_name: tool_name.clone(),
                        content_length,
                    });
                }
            }
            _ => {}
        }
    }
    
    tool_results
}

/// Identify which tool results should be truncated
fn identify_truncation_candidates(
    tool_results: &[ToolResultInfo],
    config: &TruncationConfig,
) -> BTreeSet<usize> {
    let mut candidates = BTreeSet::new();
    
    // Determine which indices to preserve
    let preserve_start = config.preserve_initial_tools;
    let preserve_end = tool_results.len().saturating_sub(config.preserve_recent_tools);
    
    // Add truncation candidates (skipping preserved indices)
    for i in preserve_start..preserve_end {
        // Prioritize truncating larger outputs first
        if tool_results[i].content_length > 500 {
            candidates.insert(tool_results[i].index);
        }
    }
    
    // If we still need more candidates, add medium-sized outputs
    if candidates.len() < (preserve_end - preserve_start) / 2 {
        for i in preserve_start..preserve_end {
            if !candidates.contains(&tool_results[i].index) && tool_results[i].content_length > 200 {
                candidates.insert(tool_results[i].index);
            }
        }
    }
    
    candidates
}

/// Apply truncation to conversation messages by replacing content with placeholders
///
/// # Arguments
/// * `messages` - Conversation messages to modify
/// * `indices_to_truncate` - Set of message indices to truncate
/// * `config` - Truncation configuration
///
/// # Returns
/// Information about the truncation performed
fn apply_truncation(
    messages: &mut Vec<Message>,
    indices_to_truncate: &BTreeSet<usize>,
    config: &TruncationConfig,
) -> TruncationResult {
    let mut truncated_count = 0;
    let mut estimated_tokens_saved = 0;
    
    for &idx in indices_to_truncate {
        if idx < messages.len() {
            // Replace the content with a placeholder while keeping the message structure
            if let Content::Text { ref mut text } = messages[idx].content {
                // Save the original length for estimating tokens saved
                let original_length = text.len();
                
                // Create truncated text with header and footer
                let (header, footer) = extract_header_footer(text);
                
                let truncated_text = format!(
                    "{}\n{}\n{}",
                    header,
                    config.placeholder_text,
                    footer
                );
                
                // Replace the text
                *text = truncated_text;
                
                // Count this truncation
                truncated_count += 1;
                
                // Estimate tokens saved (rough approximation: ~4 chars per token)
                let chars_saved = original_length.saturating_sub(text.len());
                estimated_tokens_saved += chars_saved / 4;
            }
        }
    }
    
    TruncationResult {
        truncated_messages: truncated_count,
        estimated_tokens_saved,
        truncated_indices: indices_to_truncate.clone(),
    }
}

/// Extract header and footer from a tool result output
///
/// Header typically contains the tool result tag and first line
/// Footer typically contains the closing tag
fn extract_header_footer(text: &str) -> (String, String) {
    let lines: Vec<&str> = text.lines().collect();
    
    // Extract header (first line with tool result tag)
    let header = if !lines.is_empty() {
        lines[0].to_string()
    } else {
        String::new()
    };
    
    // Extract footer (last line with closing tag)
    let footer = if lines.len() > 1 {
        lines.last().unwrap_or(&"").to_string()
    } else {
        String::new()
    };
    
    (header, footer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{Content, Message, MessageInfo};
    
    #[test]
    fn test_should_truncate() {
        let config = TruncationConfig::default();
        let safe_limit = 1000;
        
        // Below threshold
        let below_usage = TokenUsage {
            input_tokens: 700,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert!(!should_truncate(&below_usage, safe_limit, &config));
        
        // Above threshold
        let above_usage = TokenUsage {
            input_tokens: 850,
            output_tokens: 100,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert!(should_truncate(&above_usage, safe_limit, &config));
    }
    
    // Additional tests could be added here
}