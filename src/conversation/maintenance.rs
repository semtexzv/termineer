//! Conversation maintenance module
//!
//! This module provides functionality to maintain clean conversation structure
//! by removing empty content blocks and messages without meaningful content.
//!
//! This helps:
//! - Reduce token usage by eliminating unnecessary elements
//! - Improve LLM processing by ensuring clean, meaningful conversation structure
//! - Prevent errors from malformed message content

use crate::llm::{Content, Message};

/// Check if message content is empty or lacks meaningful content
///
/// # Arguments
/// * `content` - The message content to check
///
/// # Returns
/// `true` if the content is empty or lacks meaningful information
pub fn is_empty_content(content: &Content) -> bool {
    match content {
        // Empty or whitespace-only text
        Content::Text { text } => text.trim().is_empty(),

        // Thinking without actual content
        Content::Thinking { thinking, .. } => {
            thinking.as_ref().map_or(true, |t| t.trim().is_empty())
        }

        // RedactedThinking without data
        Content::RedactedThinking { data } => data.as_ref().map_or(true, |d| d.trim().is_empty()),

        // Image without source
        Content::Image { source } => source.trim().is_empty(),

        // Document without source
        Content::Document { source } => source.trim().is_empty(),
    }
}

/// Sanitize conversation by removing empty messages and content blocks
///
/// # Arguments
/// * `conversation` - The conversation messages to sanitize
///
/// # Returns
/// The number of messages that were removed
pub fn sanitize_conversation(conversation: &mut Vec<Message>) -> usize {
    let original_len = conversation.len();

    // Filter out messages that have empty content
    conversation.retain(|message| !is_empty_content(&message.content));

    // Return how many messages were removed
    original_len - conversation.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::MessageInfo;

    #[test]
    fn test_is_empty_content_text() {
        // Empty text
        let empty = Content::Text {
            text: "".to_string(),
        };
        assert!(is_empty_content(&empty));

        // Whitespace-only text
        let whitespace = Content::Text {
            text: "   \n  \t  ".to_string(),
        };
        assert!(is_empty_content(&whitespace));

        // Non-empty text
        let content = Content::Text {
            text: "Hello".to_string(),
        };
        assert!(!is_empty_content(&content));
    }

    #[test]
    fn test_is_empty_content_thinking() {
        // Thinking with None
        let none_thinking = Content::Thinking {
            thinking: None,
            signature: None,
        };
        assert!(is_empty_content(&none_thinking));

        // Thinking with empty string
        let empty_thinking = Content::Thinking {
            thinking: Some("".to_string()),
            signature: Some("test".to_string()),
        };
        assert!(is_empty_content(&empty_thinking));

        // Thinking with content
        let filled_thinking = Content::Thinking {
            thinking: Some("Some reasoning".to_string()),
            signature: None,
        };
        assert!(!is_empty_content(&filled_thinking));
    }

    #[test]
    fn test_sanitize_conversation() {
        let mut conversation = vec![
            // Good message
            Message::text("user", "Hello".to_string(), MessageInfo::User),
            // Empty message
            Message::text("assistant", "".to_string(), MessageInfo::Assistant),
            // Whitespace-only message
            Message::text("user", "  \n  ".to_string(), MessageInfo::User),
            // Good message
            Message::text("assistant", "Response".to_string(), MessageInfo::Assistant),
        ];

        let removed = sanitize_conversation(&mut conversation);

        assert_eq!(removed, 2); // Should remove 2 messages
        assert_eq!(conversation.len(), 2); // Should have 2 remaining

        // Check content manually since Content doesn't implement PartialEq
        if let Content::Text { text } = &conversation[0].content {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected Text content");
        }

        if let Content::Text { text } = &conversation[1].content {
            assert_eq!(text, "Response");
        } else {
            panic!("Expected Text content");
        }
    }
}
