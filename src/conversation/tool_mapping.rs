//! Tool mapping functionality
//!
//! This module provides structures and functions for mapping between tool
//! invocations and their corresponding results in the conversation history.
//! This enables easy retrieval of tool call/result pairs and analysis of tool usage.

use crate::llm::{Message, MessageInfo};
use std::collections::HashMap;

/// Represents a complete tool interaction (call and result)
#[derive(Debug, Clone)]
pub struct ToolInteraction {
    /// Index of the tool call message in the conversation
    pub call_index: usize,
    
    /// Index of the tool result/error message in the conversation
    pub result_index: usize,
    
    /// Name of the tool that was called
    pub tool_name: String,
    
    /// Unique identifier for this tool interaction
    pub tool_index: usize,
    
    /// Whether the tool execution was successful
    pub success: bool,
}

impl ToolInteraction {
    /// Check if this interaction has a specific tool name
    pub fn has_tool_name(&self, name: &str) -> bool {
        self.tool_name == name
    }
}

/// Manages and provides access to tool interactions within a conversation
#[derive(Debug, Default)]
pub struct ToolMapper {
    /// Map from tool index to complete interaction
    interactions: HashMap<usize, ToolInteraction>,
    
    /// Map from message index to tool index (for quick lookups)
    message_to_tool: HashMap<usize, usize>,
    
    /// Pending tool calls that don't have results yet
    pending_calls: HashMap<usize, (usize, String)>, // tool_index -> (msg_index, tool_name)
}

impl ToolMapper {
    /// Create a new empty tool mapper
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Build a tool mapper from an existing conversation
    pub fn from_conversation(conversation: &[Message]) -> Self {
        let mut mapper = Self::new();
        mapper.process_conversation(conversation);
        mapper
    }
    
    /// Process a conversation to build the tool mapping
    pub fn process_conversation(&mut self, conversation: &[Message]) {
        self.interactions.clear();
        self.message_to_tool.clear();
        self.pending_calls.clear();
        
        // Process each message in the conversation
        for (i, message) in conversation.iter().enumerate() {
            match &message.info {
                MessageInfo::ToolCall { tool_name, tool_index } => {
                    if let Some(idx) = tool_index {
                        // Store as a pending call
                        self.pending_calls.insert(*idx, (i, tool_name.clone()));
                        // Map message index to tool index
                        self.message_to_tool.insert(i, *idx);
                    }
                },
                MessageInfo::ToolResult { tool_name: _, tool_index } |
                MessageInfo::ToolError { tool_name: _, tool_index } => {
                    let is_success = matches!(message.info, MessageInfo::ToolResult { .. });
                    
                    if let Some(idx) = tool_index {
                        // Map message index to tool index
                        self.message_to_tool.insert(i, *idx);
                        
                        // Check if we have a pending call for this tool
                        if let Some((call_idx, tool_name)) = self.pending_calls.remove(idx) {
                            // Create a complete interaction
                            let interaction = ToolInteraction {
                                call_index: call_idx,
                                result_index: i,
                                tool_name,
                                tool_index: *idx,
                                success: is_success,
                            };
                            
                            // Store the interaction
                            self.interactions.insert(*idx, interaction);
                        }
                    }
                },
                _ => {} // Ignore other message types
            }
        }
    }
    
    /// Update the mapping with a new message
    pub fn process_message(&mut self, index: usize, message: &Message) {
        match &message.info {
            MessageInfo::ToolCall { tool_name, tool_index } => {
                if let Some(idx) = tool_index {
                    // Store as a pending call
                    self.pending_calls.insert(*idx, (index, tool_name.clone()));
                    // Map message index to tool index
                    self.message_to_tool.insert(index, *idx);
                }
            },
            MessageInfo::ToolResult { tool_name: _, tool_index } |
            MessageInfo::ToolError { tool_name: _, tool_index } => {
                let is_success = matches!(message.info, MessageInfo::ToolResult { .. });
                
                if let Some(idx) = tool_index {
                    // Map message index to tool index
                    self.message_to_tool.insert(index, *idx);
                    
                    // Check if we have a pending call for this tool
                    if let Some((call_idx, tool_name)) = self.pending_calls.remove(idx) {
                        // Create a complete interaction
                        let interaction = ToolInteraction {
                            call_index: call_idx,
                            result_index: index,
                            tool_name,
                            tool_index: *idx,
                            success: is_success,
                        };
                        
                        // Store the interaction
                        self.interactions.insert(*idx, interaction);
                    }
                }
            },
            _ => {} // Ignore other message types
        }
    }
    
    /// Get all complete tool interactions
    pub fn get_interactions(&self) -> Vec<&ToolInteraction> {
        let mut interactions: Vec<&ToolInteraction> = self.interactions.values().collect();
        
        // Sort by tool index to ensure consistent ordering
        interactions.sort_by_key(|i| i.tool_index);
        
        interactions
    }
    
    /// Get a specific tool interaction by tool index
    pub fn get_interaction(&self, tool_index: usize) -> Option<&ToolInteraction> {
        self.interactions.get(&tool_index)
    }
    
    /// Get the tool interaction associated with a message
    pub fn get_interaction_for_message(&self, message_index: usize) -> Option<&ToolInteraction> {
        self.message_to_tool
            .get(&message_index)
            .and_then(|idx| self.interactions.get(idx))
    }
    
    /// Get all tool interactions for a specific tool name
    pub fn get_interactions_by_name(&self, tool_name: &str) -> Vec<&ToolInteraction> {
        let mut interactions: Vec<&ToolInteraction> = self.interactions
            .values()
            .filter(|i| i.has_tool_name(tool_name))
            .collect();
        
        // Sort by tool index to ensure consistent ordering
        interactions.sort_by_key(|i| i.tool_index);
        
        interactions
    }
    
    /// Get all pending tool calls that don't have results yet
    pub fn get_pending_calls(&self) -> Vec<(usize, &str, usize)> {
        self.pending_calls
            .iter()
            .map(|(idx, (msg_idx, name))| (*idx, name.as_str(), *msg_idx))
            .collect()
    }
    
    /// Get the number of completed tool interactions
    pub fn interaction_count(&self) -> usize {
        self.interactions.len()
    }
    
    /// Get the number of pending tool calls
    pub fn pending_count(&self) -> usize {
        self.pending_calls.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_tool_call(index: usize, name: &str) -> Message {
        Message {
            role: "assistant".to_string(),
            content: crate::llm::Content::Text { 
                text: format!("Calling tool {}", name) 
            },
            info: MessageInfo::ToolCall { 
                tool_name: name.to_string(), 
                tool_index: Some(index) 
            },
        }
    }
    
    fn create_tool_result(index: usize) -> Message {
        Message {
            role: "user".to_string(),
            content: crate::llm::Content::Text { 
                text: "Tool result".to_string() 
            },
            info: MessageInfo::ToolResult { 
                tool_name: "test".to_string(), 
                tool_index: Some(index) 
            },
        }
    }
    
    fn create_tool_error(index: usize) -> Message {
        Message {
            role: "user".to_string(),
            content: crate::llm::Content::Text { 
                text: "Tool error".to_string() 
            },
            info: MessageInfo::ToolError { 
                tool_name: "test".to_string(), 
                tool_index: Some(index) 
            },
        }
    }
    
    #[test]
    fn test_process_conversation() {
        let conversation = vec![
            create_tool_call(1, "read"),
            create_tool_result(1),
            create_tool_call(2, "shell"),
            create_tool_error(2),
        ];
        
        let mapper = ToolMapper::from_conversation(&conversation);
        
        assert_eq!(mapper.interaction_count(), 2);
        assert_eq!(mapper.pending_count(), 0);
        
        // Check first interaction
        let interaction1 = mapper.get_interaction(1).unwrap();
        assert_eq!(interaction1.call_index, 0);
        assert_eq!(interaction1.result_index, 1);
        assert_eq!(interaction1.tool_name, "read");
        assert!(interaction1.success);
        
        // Check second interaction
        let interaction2 = mapper.get_interaction(2).unwrap();
        assert_eq!(interaction2.call_index, 2);
        assert_eq!(interaction2.result_index, 3);
        assert_eq!(interaction2.tool_name, "shell");
        assert!(!interaction2.success);
    }
    
    #[test]
    fn test_process_message() {
        let mut mapper = ToolMapper::new();
        
        // Process tool call
        mapper.process_message(0, &create_tool_call(1, "read"));
        assert_eq!(mapper.interaction_count(), 0);
        assert_eq!(mapper.pending_count(), 1);
        
        // Process tool result
        mapper.process_message(1, &create_tool_result(1));
        assert_eq!(mapper.interaction_count(), 1);
        assert_eq!(mapper.pending_count(), 0);
        
        // Check the interaction
        let interaction = mapper.get_interaction(1).unwrap();
        assert_eq!(interaction.call_index, 0);
        assert_eq!(interaction.result_index, 1);
        assert_eq!(interaction.tool_name, "read");
        assert!(interaction.success);
    }
    
    #[test]
    fn test_get_interactions_by_name() {
        let conversation = vec![
            create_tool_call(1, "read"),
            create_tool_result(1),
            create_tool_call(2, "read"),
            create_tool_result(2),
            create_tool_call(3, "shell"),
            create_tool_result(3),
        ];
        
        let mapper = ToolMapper::from_conversation(&conversation);
        
        // Get all "read" interactions
        let read_interactions = mapper.get_interactions_by_name("read");
        assert_eq!(read_interactions.len(), 2);
        assert_eq!(read_interactions[0].tool_index, 1);
        assert_eq!(read_interactions[1].tool_index, 2);
        
        // Get all "shell" interactions
        let shell_interactions = mapper.get_interactions_by_name("shell");
        assert_eq!(shell_interactions.len(), 1);
        assert_eq!(shell_interactions[0].tool_index, 3);
    }
    
    #[test]
    fn test_get_interaction_for_message() {
        let conversation = vec![
            create_tool_call(1, "read"),
            create_tool_result(1),
        ];
        
        let mapper = ToolMapper::from_conversation(&conversation);
        
        // Get interaction for the call message
        let interaction1 = mapper.get_interaction_for_message(0).unwrap();
        assert_eq!(interaction1.tool_index, 1);
        
        // Get interaction for the result message
        let interaction2 = mapper.get_interaction_for_message(1).unwrap();
        assert_eq!(interaction2.tool_index, 1);
        
        // Should be the same interaction
        assert_eq!(interaction1.tool_index, interaction2.tool_index);
    }
}