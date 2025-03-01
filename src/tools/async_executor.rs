use crate::tools::ToolResult;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;

pub enum ToolMessage {
    Line(String),
    Complete(ToolResult),
}

/// Trait for tools that support asynchronous execution with streaming output
pub trait AsyncTool {
    fn execute_async(&self, tool_content: String) -> (Receiver<ToolMessage>, thread::JoinHandle<()>);
}

/// Execute a shell command with streaming output
pub fn execute_with_streaming<F>(
    tool_content: String,
    executor: F,
) -> (Receiver<ToolMessage>, thread::JoinHandle<()>)
where
    F: FnOnce(String, Sender<ToolMessage>) + Send + 'static,
{
    let (tx, rx) = channel();
    
    // Execute the function in a new thread
    let handle = thread::spawn(move || {
        executor(tool_content, tx);
    });
    
    (rx, handle)
}