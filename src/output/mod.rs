//! Output buffer system using shared mutex-protected queues
//!
//! This module provides a buffer system where each task has its own output buffer
//! accessed through task-local storage, allowing for clean API with no buffer passing.

use chrono::Utc;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::task;

/// Types of output lines that can be stored in the buffer
#[derive(Debug, Clone, PartialEq)]
pub enum OutputType {
    /// Standard output (normal messages)
    Standard,
    /// Error messages
    Error,
    /// Tool output messages
    Tool(String),
    /// System messages (application status, etc.)
    System,
    /// Debug messages (only shown in verbose mode)
    Debug,
}

/// A single line of output with its type
#[derive(Debug, Clone)]
pub struct OutputLine {
    /// The type of output
    pub output_type: OutputType,
    /// The actual text content
    pub content: String,
    /// Optional formatting (e.g., ANSI color codes)
    pub formatting: Option<String>,
    /// Timestamp when the line was added
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Shared buffer queue protected by a mutex
#[derive(Debug, Clone)]
pub struct SharedBuffer {
    /// The mutex-protected queue of output lines
    queue: Arc<Mutex<VecDeque<OutputLine>>>,
}

impl SharedBuffer {
    /// Create a new shared buffer with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
        }
    }
    pub fn lines(&self) -> MutexGuard<VecDeque<OutputLine>> {
        self.queue.lock().unwrap()
    }

    /// Push a line to the buffer
    pub fn push(&self, line: OutputLine) -> Result<(), String> {
        match self.queue.lock() {
            Ok(mut queue) => {
                queue.push_back(line);
                Ok(())
            }
            Err(e) => Err(format!("Failed to lock buffer queue: {}", e)),
        }
    }

    /// Pop a line from the buffer
    pub fn pop(&self) -> Option<OutputLine> {
        match self.queue.lock() {
            Ok(mut queue) => queue.pop_front(),
            Err(e) => {
                eprintln!("Failed to lock buffer queue: {}", e);
                None
            }
        }
    }

    /// Send a line to the buffer
    pub fn send(
        &self,
        output_type: OutputType,
        content: impl Into<String>,
        formatting: Option<String>,
    ) -> Result<(), String> {
        let line = OutputLine {
            output_type,
            content: content.into(),
            formatting,
            timestamp: Utc::now(),
        };

        self.push(line)
    }

    /// Add standard output line(s), splitting by newlines
    pub fn stdout(&self, content: impl Into<String>) -> Result<(), String> {
        self.send_split_lines(OutputType::Standard, content.into(), None)
    }

    /// Add error output line(s), splitting by newlines
    pub fn stderr(&self, content: impl Into<String>) -> Result<(), String> {
        self.send_split_lines(OutputType::Error, content.into(), None)
    }

    /// Add tool output line(s), splitting by newlines
    pub fn tool(
        &self,
        tool_name: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<(), String> {
        self.send_split_lines(OutputType::Tool(tool_name.into()), content.into(), None)
    }
    
    /// Helper method to split content by newlines and add each line separately
    fn send_split_lines(
        &self,
        output_type: OutputType,
        content: String,
        formatting: Option<String>,
    ) -> Result<(), String> {
        // Split the content by newlines
        let lines = content.split('\n');
        
        // Add each line as a separate OutputLine
        for line in lines {
            // Skip empty lines if they're at the end
            if line.is_empty() {
                continue;
            }
            
            let output_line = OutputLine {
                output_type: output_type.clone(),
                content: line.to_string(),
                formatting: formatting.clone(),
                timestamp: Utc::now(),
            };
            
            self.push(output_line)?;
        }
        
        Ok(())
    }

}

// Task-local storage for the current task's output buffer
tokio::task_local! {
    pub static CURRENT_BUFFER: SharedBuffer;
}

/// Spawn a new tokio task with an output buffer in task-local storage
pub fn spawn_with_buffer<F, T>(buffer: SharedBuffer, future: F) -> task::JoinHandle<T>
where
    F: futures::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(CURRENT_BUFFER.scope(buffer, async move { future.await }))
}
