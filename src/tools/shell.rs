use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::process::Command;
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal;
use crossterm::terminal::disable_raw_mode;
use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};

/// Data structure for managing interruption with reason
pub struct InterruptData {
    /// Flag indicating whether the process should be interrupted
    pub flag: bool,
    /// Optional reason for the interruption
    pub reason: Option<String>,
}

impl InterruptData {
    /// Create a new InterruptData instance
    pub fn new() -> Self {
        Self {
            flag: false,
            reason: None,
        }
    }

    /// Set interruption with reason
    pub fn interrupt(&mut self, reason: String) {
        self.flag = true;
        self.reason = Some(reason);
    }

    /// Check if interruption is requested
    pub fn is_interrupted(&self) -> bool {
        self.flag
    }

    /// Get interruption reason
    pub fn reason(&self) -> Option<&String> {
        self.reason.as_ref()
    }
}

/// Message type for shell output streaming
pub enum ShellOutput {
    /// Line from standard output
    Stdout(String),
    /// Line from standard error
    Stderr(String),
    /// Completion signal with final result
    Complete(ToolResult),
}

/// Execute shell command with streaming output and interruption capability
/// Returns a receiver to consume streaming output
///
/// # Arguments
/// * `command_to_run` - Command to execute
/// * `body` - Optional script body (overrides command if not empty)
/// * `interrupt_data` - Shared data for interruption coordination
/// * `silent_mode` - Whether to suppress console output
///
/// # Returns
/// A receiver for consuming output events and final result
pub async fn execute_shell(
    command_to_run: &str,
    body: &str,
    interrupt_data: Arc<Mutex<InterruptData>>, 
    silent_mode: bool
) -> Result<mpsc::Receiver<ShellOutput>, Box<dyn std::error::Error>> {
    // If body is provided, use it as a script instead of the args
    let command_str = if !body.is_empty() {
        body.to_string()
    } else {
        command_to_run.to_string()
    };

    let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
    let shell_arg = if cfg!(target_os = "windows") { "/C" } else { "-c" };

    // Create a channel for output streaming
    let (sender, receiver) = mpsc::channel(100); // Buffer size of 100 messages
    
    // Print initial status message
    if !silent_mode {
        // Ensure terminal is in normal mode for console output
        let _ = disable_raw_mode();
        
        println!("{}🐚 Shell:{} {} (streaming - can be interrupted)", 
                FORMAT_BOLD, FORMAT_RESET, command_str);
    }
    
    // Clone the interrupt data for thread use
    let thread_interrupt_data = Arc::clone(&interrupt_data);
    
    // Start the actual command
    let mut child = Command::new(shell)
        .arg(shell_arg)
        .arg(&command_str)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
            
    // Take the stdout and stderr handles
    let stdout = child.stdout.take().expect("Failed to capture stdout");
    let stderr = child.stderr.take().expect("Failed to capture stderr");

    // Command status (using tokio::sync::Mutex now)
    let command_running = Arc::new(tokio::sync::Mutex::new(true));
    
    // Interrupt data clone for checking
    let interrupt_data_clone = Arc::clone(&thread_interrupt_data);
    
    // Stdout reader task
    let stdout_sender = sender.clone();
    let stdout_running_clone = Arc::clone(&command_running);
    let stdout_silent = silent_mode;
    
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut line_count = 0;
        
        while let Ok(Some(line)) = reader.next_line().await {
            // Display line if not in silent mode
            if !stdout_silent {
                print!("{}{}{}\r\n", FORMAT_GRAY, line, FORMAT_RESET);
                
                // Flush periodically for better performance
                line_count += 1;
                if line_count % 10 == 0 {
                    std::io::stdout().flush().unwrap_or(());
                }
            }
            
            // Send the line through the channel
            if stdout_sender.send(ShellOutput::Stdout(line.clone())).await.is_err() {
                break;
            }
            
            // Check if we should exit
            if !*stdout_running_clone.lock().await {
                break;
            }
        }
        
        // Ensure any remaining output is flushed at the end
        if !stdout_silent {
            std::io::stdout().flush().unwrap_or(());
        }
    });
    
    // Stderr reader task
    let stderr_sender = sender.clone();
    let stderr_running_clone = Arc::clone(&command_running);
    let stderr_silent = silent_mode;
    
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut line_count = 0;
        
        while let Ok(Some(line)) = reader.next_line().await {
            // Display line if not in silent mode
            if !stderr_silent {
                print!("{}{}{}\r\n", FORMAT_GRAY, line, FORMAT_RESET);
                
                // Flush periodically for better performance
                line_count += 1;
                if line_count % 10 == 0 {
                    std::io::stdout().flush().unwrap_or(());
                }
            }
            
            // Send the line through the channel
            if stderr_sender.send(ShellOutput::Stderr(line.clone())).await.is_err() {
                break;
            }
            
            // Check if we should exit
            if !*stderr_running_clone.lock().await {
                break;
            }
        }
        
        // Ensure any remaining output is flushed at the end
        if !stderr_silent {
            std::io::stdout().flush().unwrap_or(());
        }
    });
    
    // Create a task to handle keyboard interruptions
    if !silent_mode {
        let keyboard_interrupt_data = Arc::clone(&interrupt_data_clone);
        let keyboard_command_running = Arc::clone(&command_running);
        
        tokio::spawn(async move {
            loop {
                // Check for Ctrl+C with a longer poll time to not miss keypresses
                // Note: This still uses std blocking calls as crossterm isn't async
                if terminal::enable_raw_mode().is_ok() && event::poll(std::time::Duration::from_millis(10)).unwrap_or(false) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            // Set interrupt with reason
                            let mut data = keyboard_interrupt_data.lock().unwrap();
                            data.interrupt("User interrupted command with Ctrl+C".to_string());

                            // Exit the keyboard handling task
                            break;
                        }
                    }
                }

                // Check if the process is already done
                {
                    let data = keyboard_interrupt_data.lock().unwrap();
                    if data.is_interrupted() {
                        break; // Process was interrupted or completed
                    }
                }
                
                // Check if command finished on its own
                if !*keyboard_command_running.lock().await {
                    let _ = disable_raw_mode();
                    break;
                }
                
                // Small sleep to avoid CPU spinning
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });
    }
    
    // Spawn the main monitoring task
    let main_command_str = command_str.clone();
    let main_sender = sender.clone();
    let main_silent_mode = silent_mode;
    
    tokio::spawn(async move {
        // Main process monitoring variables
        let mut was_interrupted = false;
        let mut interrupt_reason = String::new();
        let mut exit_status = None;
        
        // Main process monitoring loop
        loop {
            // Check if process has completed on its own
            match child.try_wait() {
                Ok(Some(status)) => {
                    // Store the exit status for later use
                    exit_status = Some(status);
                    *command_running.lock().await = false;
                    break;
                }
                Ok(None) => {
                    // Check if interruption requested
                    let is_interrupted;
                    {
                        let interrupt_data = interrupt_data_clone.lock().unwrap();
                        is_interrupted = interrupt_data.is_interrupted();
                        if is_interrupted {
                            // Get the reason if available
                            if let Some(reason) = interrupt_data.reason() {
                                interrupt_reason = reason.clone();
                            } else {
                                interrupt_reason = "No reason provided".to_string();
                            }
                        }
                    }
                    
                    if is_interrupted {
                        // Kill the process
                        was_interrupted = true;
                        let _ = child.kill().await;
                        *command_running.lock().await = false;
                        break;
                    }
                }
                Err(e) => {
                    // Error checking process
                    *command_running.lock().await = false;
                    
                    // Log error
                    if !main_silent_mode {
                        // Ensure terminal is in normal mode
                        let _ = disable_raw_mode();
                        
                        println!("{}🐚 Shell:{} Error monitoring process: {}", 
                            FORMAT_BOLD, FORMAT_RESET, e);
                    }
                    
                    // Send error completion
                    let _ = main_sender.send(ShellOutput::Complete(ToolResult {
                        success: false,
                        agent_output: format!("Error monitoring process status: {}", e),
                    })).await;
                    return;
                }
            }
            
            // Brief delay to avoid CPU spinning
            sleep(Duration::from_millis(10)).await;
        }

        // Wait a moment for stdout/stderr to finish processing
        sleep(Duration::from_millis(50)).await;

        let _ = disable_raw_mode();

        // Determine success based on exit status or interruption
        let success = if was_interrupted {
            true // Interruption is successful
        } else {
            // Use the stored exit status
            exit_status.map_or(false, |status| status.success())
        };

        // Combined output
        let agent_output = if was_interrupted {
            format!(
                "Command '{}' was interrupted: {}.",
                main_command_str, interrupt_reason,
            )
        } else {
            if success {
                format!("Command '{}' finished with success", main_command_str)
            } else {
                format!("Command '{}' finished with error", main_command_str)
            }
        };

        // Print status message
        if !main_silent_mode {
            // Ensure we're in normal mode for console output
            let _ = disable_raw_mode();
            
            if was_interrupted {
                println!("{}🐚 Shell:{} {} (interrupted: {})",
                    FORMAT_BOLD, FORMAT_RESET, main_command_str, interrupt_reason);
            } else if success {
                println!("{}🐚 Shell:{} {} (completed successfully)", 
                    FORMAT_BOLD, FORMAT_RESET, main_command_str);
            } else {
                println!("{}🐚 Shell:{} {} (completed with error)",
                    FORMAT_BOLD, FORMAT_RESET, main_command_str);
            }
        }

        // Send final completion message with result
        let _ = main_sender.send(ShellOutput::Complete(ToolResult {
            success,
            agent_output,
        })).await;
    });
    
    // Return the receiver for streaming output
    Ok(receiver)
}