//! Token management and tracking
//!
//! This module provides functionality for tracking token usage,
//! managing tool output relevance, and optimizing token usage.

use crate::llm::{Backend, Message, TokenUsage};
use std::collections::HashMap;

/// Token limit thresholds for debugging (extremely low to trigger token management)
/// Original values commented below for easy restoration
pub const TOKEN_LIMIT_WARNING_THRESHOLD: usize = 5000;  // Debug: Trigger warnings with minimal usage
pub const TOKEN_LIMIT_CRITICAL_THRESHOLD: usize = 8000; // Debug: Trigger critical with minimal usage
pub const MAX_EXPECTED_OUTPUT_TOKENS: usize = 1000;     // Debug: Minimal output reservation

// Original values for Claude's 200k context window:
// pub const TOKEN_LIMIT_WARNING_THRESHOLD: usize = 160000; // 80% of 200k tokens
// pub const TOKEN_LIMIT_CRITICAL_THRESHOLD: usize = 180000; // 90% of 200k tokens
// pub const MAX_EXPECTED_OUTPUT_TOKENS: usize = 10000; // Reserve space for model's response

/// Metadata about a tool output for token management
#[derive(Debug, Clone)]
pub struct ToolOutputMetadata {
    /// Whether this tool output is still relevant
    pub relevant: bool,
    
    /// Estimated input tokens for this tool result
    pub input_tokens: Option<usize>,
    
    /// The tool name
    pub tool_name: String,
}

/// Token management interface for tracking and optimizing token usage
pub struct TokenManager {
    /// Tracks tool output metadata including relevance and token usage
    pub tool_metadata: HashMap<usize, ToolOutputMetadata>,
}

impl TokenManager {
    /// Create a new token manager
    pub fn new() -> Self {
        Self {
            tool_metadata: HashMap::new(),
        }
    }
    
    /// Count tokens for a list of messages using the LLM backend
    pub async fn count_tokens<B: Backend + ?Sized>(
        messages: &[Message],
        system: Option<&str>,
        backend: &B,
    ) -> Result<TokenUsage, Box<dyn std::error::Error + Send + Sync>> {
        match backend.count_tokens(messages, system).await {
            Ok(usage) => Ok(usage),
            Err(e) => Err(format!("Failed to count tokens: {}", e).into()),
        }
    }
    
    /// Register a new tool output with the token manager
    pub fn register_tool_output(
        &mut self,
        tool_index: usize,
        tool_name: String,
    ) {
        self.tool_metadata.insert(tool_index, ToolOutputMetadata {
            relevant: true,
            input_tokens: None,
            tool_name,
        });
    }
    
    /// Update token usage for a tool output
    pub fn update_token_usage(
        &mut self,
        tool_index: usize,
        input_tokens: Option<usize>,
        output_tokens: Option<usize>,
    ) {
        if let Some(metadata) = self.tool_metadata.get_mut(&tool_index) {
            metadata.input_tokens = input_tokens;
        }
    }
    
    /// Mark a tool output as irrelevant
    pub fn mark_irrelevant(&mut self, tool_index: usize) {
        if let Some(metadata) = self.tool_metadata.get_mut(&tool_index) {
            metadata.relevant = false;
        }
    }
    
    /// Check if a tool output is marked as irrelevant
    pub fn is_irrelevant(&self, tool_index: usize) -> bool {
        self.tool_metadata.get(&tool_index)
            .map_or(false, |metadata| !metadata.relevant)
    }
    
    /// Get all tool indices tracked by the token manager
    pub fn get_tool_indices(&self) -> Vec<usize> {
        self.tool_metadata.keys().cloned().collect()
    }
    
    /// Get a formatted report of tool metadata for the LLM
    pub fn format_tool_details(&self, total_tokens: usize) -> String {
        let mut tool_details = String::new();
        
        for (idx, metadata) in &self.tool_metadata {
            // Format information about this tool with accurate token counts
            let token_info = if metadata.input_tokens.is_some() || metadata.output_tokens.is_some() {
                format!(
                    "{} tokens", 
                    metadata.input_tokens.unwrap_or(0) + metadata.output_tokens.unwrap_or(0)
                )
            } else {
                "token count unknown".to_string()
            };
            
            // Calculate the percentage of total tokens this tool represents
            let percentage = if let Some(input_tokens) = metadata.input_tokens {
                let total_input_tokens = input_tokens + metadata.output_tokens.unwrap_or(0);
                if total_tokens > 0 {
                    format!(" ({:.1}% of total)", (total_input_tokens as f32 / total_tokens as f32) * 100.0)
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            };
            
            tool_details.push_str(&format!(
                "Index {}: {} ({}{})\n  Status: {}\n",
                idx,
                metadata.tool_name,
                token_info,
                percentage,
                if metadata.relevant { 
                    "Currently marked as relevant" 
                } else { 
                    "Already marked for truncation" 
                }
            ));
        }
        
        tool_details
    }
}