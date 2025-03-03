//! ANSI escape sequence to ratatui Span converter
//!
//! This module handles converting text with ANSI escape sequences
//! to ratatui's Span-based formatting for use in the TUI.
//! It also provides functionality to strip ANSI sequences from text.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Represents the current styling state
#[derive(Clone, Debug, Default)]
struct StyleState {
    /// Text foreground color
    fg_color: Option<Color>,
    /// Text background color
    bg_color: Option<Color>,
    /// Is text bold
    bold: bool,
}

impl StyleState {
    /// Create a new default style state
    fn new() -> Self {
        Self::default()
    }

    /// Convert to a ratatui Style
    fn to_style(&self) -> Style {
        let mut style = Style::default();
        
        if let Some(fg) = self.fg_color {
            style = style.fg(fg);
        }
        
        if let Some(bg) = self.bg_color {
            style = style.bg(bg);
        }
        
        if self.bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        
        style
    }

    /// Reset all style attributes
    fn reset(&mut self) {
        self.fg_color = None;
        self.bg_color = None;
        self.bold = false;
    }
}

/// Convert a string with ANSI escape sequences to a ratatui Line
pub fn ansi_to_line(text: &str) -> Line<'static> {
    let mut spans = Vec::new();
    let mut current_text = String::new();
    let mut current_style = StyleState::new();
    let mut i = 0;
    
    // Convert the string to chars for easier processing
    let chars: Vec<char> = text.chars().collect();
    
    while i < chars.len() {
        // Check for escape sequence start
        if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // If we have accumulated text, add it as a span with the current style
            if !current_text.is_empty() {
                spans.push(Span::styled(current_text.clone(), current_style.to_style()));
                current_text.clear();
            }
            
            // Find the end of the escape sequence (marked by 'm')
            let mut j = i + 2;
            while j < chars.len() && chars[j] != 'm' {
                j += 1;
            }
            
            if j < chars.len() {
                // Extract the escape sequence
                let escape_seq: String = chars[i..=j].iter().collect();
                
                // Parse the escape sequence
                parse_escape_sequence(&escape_seq, &mut current_style);
                
                // Move past the escape sequence
                i = j + 1;
                continue;
            }
        }
        
        // Regular character, add to current text
        current_text.push(chars[i]);
        i += 1;
    }
    
    // Add any remaining text
    if !current_text.is_empty() {
        spans.push(Span::styled(current_text, current_style.to_style()));
    }
    
    Line::from(spans)
}

/// Convert a string with ANSI escape sequences to multiple ratatui Lines
pub fn ansi_to_lines(text: &str) -> Vec<Line<'static>> {
    text.lines()
        .map(ansi_to_line)
        .collect()
}


/// Parse an ANSI escape sequence and update the style state
fn parse_escape_sequence(sequence: &str, style: &mut StyleState) {
    // Extract the numeric part of the sequence
    if let Some(code_str) = sequence.strip_prefix("\x1b[").and_then(|s| s.strip_suffix('m')) {
        // Handle different codes
        match code_str {
            "0" => style.reset(),
            "1" => style.bold = true,
            "30" => style.fg_color = Some(Color::Black),
            "31" => style.fg_color = Some(Color::Red),
            "32" => style.fg_color = Some(Color::Green),
            "33" => style.fg_color = Some(Color::Yellow),
            "34" => style.fg_color = Some(Color::Blue),
            "35" => style.fg_color = Some(Color::Magenta),
            "36" => style.fg_color = Some(Color::Cyan),
            "37" => style.fg_color = Some(Color::White),
            "90" => style.fg_color = Some(Color::Gray),
            "40" => style.bg_color = Some(Color::Black),
            "41" => style.bg_color = Some(Color::Red),
            "42" => style.bg_color = Some(Color::Green),
            "43" => style.bg_color = Some(Color::Yellow),
            "44" => style.bg_color = Some(Color::Blue),
            "45" => style.bg_color = Some(Color::Magenta),
            "46" => style.bg_color = Some(Color::Cyan),
            "47" => style.bg_color = Some(Color::White),
            _ => {
                // Handle multiple codes separated by semicolons
                for code in code_str.split(';') {
                    match code {
                        "0" => style.reset(),
                        "1" => style.bold = true,
                        "30" => style.fg_color = Some(Color::Black),
                        "31" => style.fg_color = Some(Color::Red),
                        "32" => style.fg_color = Some(Color::Green),
                        "33" => style.fg_color = Some(Color::Yellow),
                        "34" => style.fg_color = Some(Color::Blue),
                        "35" => style.fg_color = Some(Color::Magenta),
                        "36" => style.fg_color = Some(Color::Cyan),
                        "37" => style.fg_color = Some(Color::White),
                        "90" => style.fg_color = Some(Color::Gray),
                        "40" => style.bg_color = Some(Color::Black),
                        "41" => style.bg_color = Some(Color::Red),
                        "42" => style.bg_color = Some(Color::Green),
                        "43" => style.bg_color = Some(Color::Yellow),
                        "44" => style.bg_color = Some(Color::Blue),
                        "45" => style.bg_color = Some(Color::Magenta),
                        "46" => style.bg_color = Some(Color::Cyan),
                        "47" => style.bg_color = Some(Color::White),
                        _ => {} // Ignore unsupported codes
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_plain_text() {
        let text = "Hello, world!";
        let line = ansi_to_line(text);
        
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "Hello, world!");
    }
    
    #[test]
    fn test_bold_text() {
        let text = "Hello, \x1b[1mbold\x1b[0m world!";
        let line = ansi_to_line(text);
        
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "Hello, ");
        assert_eq!(line.spans[1].content, "bold");
        assert_eq!(line.spans[2].content, " world!");
        
        // Check that the middle span has bold styling
        assert!(line.spans[1].style.add_modifier.contains(Modifier::BOLD));
    }
    
    #[test]
    fn test_colored_text() {
        let text = "Normal \x1b[31mred\x1b[0m text";
        let line = ansi_to_line(text);
        
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].content, "Normal ");
        assert_eq!(line.spans[1].content, "red");
        assert_eq!(line.spans[2].content, " text");
        
        // Check that the middle span has red color
        assert_eq!(line.spans[1].style.fg, Some(Color::Red));
    }
    
    #[test]
    fn test_multiple_styles() {
        let text = "Normal \x1b[1;31mbold red\x1b[0m text";
        let line = ansi_to_line(text);
        
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[1].content, "bold red");
        
        // Check that the middle span has red color and bold
        assert_eq!(line.spans[1].style.fg, Some(Color::Red));
        assert!(line.spans[1].style.add_modifier.contains(Modifier::BOLD));
    }
}

/// Strips ANSI escape sequences from text
///
/// This function removes all ANSI escape sequences from a text string, making it
/// suitable for sending to LLMs or other contexts where formatting should be removed.
/// 
/// ANSI escape sequences are used for terminal formatting like colors, bold text,
/// cursor movement, etc. When sending output to LLMs, these sequences should be
/// removed to improve readability and reduce token consumption.
///
/// # Arguments
/// * `text` - The text string containing ANSI escape sequences
///
/// # Returns
/// A new string with all ANSI escape sequences removed
///
/// # Implementation Note
/// This sanitizer handles all common ANSI escape sequences:
/// - Text colors (foreground and background)
/// - Text styles (bold, italic, underline, etc.)
/// - Cursor movement commands
/// - Screen clearing commands
pub fn strip_ansi_sequences(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    
    // Convert the string to chars for easier processing
    let chars: Vec<char> = text.chars().collect();
    
    while i < chars.len() {
        // Check for escape sequence start
        if chars[i] == '\x1b' && i + 1 < chars.len() && chars[i + 1] == '[' {
            // Find the end of the escape sequence (marked by one of several characters)
            // Most sequences end with 'm', but others might end with different letters
            let mut j = i + 2;
            while j < chars.len() && !chars[j].is_alphabetic() {
                j += 1;
            }
            
            if j < chars.len() {
                // Skip the entire escape sequence
                i = j + 1;
                continue;
            }
        }
        
        // Regular character, add to result
        result.push(chars[i]);
        i += 1;
    }
    
    result
}

#[cfg(test)]
mod tests_strip_ansi {
    use super::*;
    
    #[test]
    fn test_strip_plain_text() {
        let text = "Hello, world!";
        let result = strip_ansi_sequences(text);
        
        assert_eq!(result, "Hello, world!");
    }
    
    #[test]
    fn test_strip_bold_text() {
        let text = "Hello, \x1b[1mbold\x1b[0m world!";
        let result = strip_ansi_sequences(text);
        
        assert_eq!(result, "Hello, bold world!");
    }
    
    #[test]
    fn test_strip_colored_text() {
        let text = "Normal \x1b[31mred\x1b[0m text";
        let result = strip_ansi_sequences(text);
        
        assert_eq!(result, "Normal red text");
    }
    
    #[test]
    fn test_strip_multiple_styles() {
        let text = "Normal \x1b[1;31mbold red\x1b[0m text";
        let result = strip_ansi_sequences(text);
        
        assert_eq!(result, "Normal bold red text");
    }
    
    #[test]
    fn test_strip_cursor_movement() {
        let text = "Text with \x1b[2Acursor\x1b[1B movement";
        let result = strip_ansi_sequences(text);
        
        assert_eq!(result, "Text with cursor movement");
    }
}