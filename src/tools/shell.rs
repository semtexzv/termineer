use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
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
pub fn execute_shell(
    command_to_run: &str,
    body: &str,
    interrupt_data: Arc<Mutex<InterruptData>>, 
    silent_mode: bool
) -> mpsc::Receiver<ShellOutput> {
    // If body is provided, use it as a script instead of the args
    let command_str = if !body.is_empty() {
        body.to_string()
    } else {
        command_to_run.to_string()
    };

    let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
    let shell_arg = if cfg!(target_os = "windows") { "/C" } else { "-c" };

    // Create a channel for output streaming
    let (sender, receiver) = mpsc::channel();
    
    // Print initial status message
    if !silent_mode {
        // Ensure terminal is in normal mode for console output
        let _ = disable_raw_mode();
        
        println!("{}🐚 Shell:{} {} (streaming - can be interrupted)", 
                FORMAT_BOLD, FORMAT_RESET, command_str);
    }
    
    // Clone the interrupt data for thread use
    let thread_interrupt_data = Arc::clone(&interrupt_data);
    
    // Spawn the process manager in a separate thread
    thread::spawn(move || {
        // Start the actual command
        let command_result = Command::new(shell)
            .arg(shell_arg)
            .arg(&command_str)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
            
        match command_result {
            Ok(mut child) => {
                // Take the stdout and stderr handles
                let stdout = child.stdout.take().expect("Failed to capture stdout");
                let stderr = child.stderr.take().expect("Failed to capture stderr");

                // Command status
                let command_running = Arc::new(Mutex::new(true));
                let stdout_running_clone = Arc::clone(&command_running);
                let stderr_running_clone = Arc::clone(&command_running);
                
                // Interrupt data clone for checking
                let interrupt_data_clone = Arc::clone(&thread_interrupt_data);
                
                // Stdout reader thread
                let stdout_sender = sender.clone();
                let stdout_thread = thread::spawn(move || {
                    let reader = BufReader::new(stdout);
                    let mut line_count = 0;
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            // Display line if not in silent mode (without prefix)
                            if !silent_mode {
                                print!("{}{}{}\r\n", FORMAT_GRAY, line, FORMAT_RESET);
                                
                                // Flush periodically (every 10 lines) for better performance
                                line_count += 1;
                                if line_count % 10 == 0 {
                                    std::io::stdout().flush().unwrap_or(());
                                }
                            }
                            
                            // Send the line through the channel
                            let _ = stdout_sender.send(ShellOutput::Stdout(line.clone()));
                        }
                        // Check if we should exit
                        if !*stdout_running_clone.lock().unwrap() {
                            break;
                        }
                    }
                    
                    // Ensure any remaining output is flushed at the end
                    if !silent_mode {
                        std::io::stdout().flush().unwrap_or(());
                    }
                });
                
                // Stderr reader thread
                let stderr_sender = sender.clone();
                let stderr_thread = thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    let mut line_count = 0;
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            // Display line if not in silent mode (without prefix)
                            if !silent_mode {
                                print!("{}{}{}\r\n", FORMAT_GRAY, line, FORMAT_RESET);
                                
                                // Flush periodically (every 10 lines) for better performance
                                line_count += 1;
                                if line_count % 10 == 0 {
                                    std::io::stdout().flush().unwrap_or(());
                                }
                            }
                            
                            // Send the line through the channel
                            let _ = stderr_sender.send(ShellOutput::Stderr(line.clone()));
                        }
                        
                        // Check if we should exit
                        if !*stderr_running_clone.lock().unwrap() {
                            break;
                        }
                    }
                    
                    // Ensure any remaining output is flushed at the end
                    if !silent_mode {
                        std::io::stdout().flush().unwrap_or(());
                    }
                });
                
                // Main process monitoring loop with keyboard interrupt handling
                let mut was_interrupted = false;
                let mut interrupt_reason = String::new();
                let mut exit_status = None;
                
                // Create a thread to handle keyboard interruptions
                if !silent_mode {
                    let keyboard_interrupt_data = Arc::clone(&interrupt_data_clone);
                    let command_running = Arc::clone(&command_running);
                    thread::spawn(move || {
                        loop {
                            // Check for Ctrl+C with a longer poll time to not miss keypresses
                            if terminal::enable_raw_mode().is_ok() && event::poll(Duration::from_millis(10)).unwrap_or(false) {
                                if let Ok(Event::Key(key)) = event::read() {
                                    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {

                                        // Set interrupt with reason
                                        let mut data = keyboard_interrupt_data.lock().unwrap();
                                        data.interrupt("User interrupted command with Ctrl+C".to_string());

                                        // Exit the keyboard handling thread
                                        return;
                                    }
                                }
                            }

                            // Check if the process is already done
                            {
                                let data = keyboard_interrupt_data.lock().unwrap();
                                if data.is_interrupted() {
                                    return; // Process was interrupted or completed
                                }
                            }
                            // Check if command finished on its own
                            {
                                if !*command_running.lock().unwrap() {
                                    let _ = disable_raw_mode();
                                    return
                                }
                            }
                        }
                    });
                }
                
                // Main process monitoring loop
                loop {
                    // Check if process has completed on its own
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            // Store the exit status for later use
                            exit_status = Some(status);
                            *command_running.lock().unwrap() = false;
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
                                let _ = child.kill();
                                *command_running.lock().unwrap() = false;
                                break;
                            }
                        }
                        Err(e) => {
                            // Error checking process
                            *command_running.lock().unwrap() = false;
                            
                            // Log error
                            if !silent_mode {
                                // Ensure terminal is in normal mode
                                let _ = disable_raw_mode();
                                
                                println!("{}🐚 Shell:{} Error monitoring process: {}", 
                                    FORMAT_BOLD, FORMAT_RESET, e);
                            }
                            
                            // Send error completion
                            let _ = sender.send(ShellOutput::Complete(ToolResult {
                                success: false,
                                agent_output: format!("Error monitoring process status: {}", e),
                            }));
                            return;
                        }
                    }
                    
                    // Brief delay to avoid CPU spinning
                    thread::sleep(Duration::from_millis(10));
                }

                // Determine success based on exit status or interruption
                let success = if was_interrupted {
                    true // Interruption is successful
                } else {
                    // Use the stored exit status
                    exit_status.map_or(false, |status| status.success())
                };

                // Wait for stdout/stderr threads to finish
                let _ = stdout_thread.join();
                let _ = stderr_thread.join();

                let _ = disable_raw_mode();

                // Combined output
                let agent_output = if was_interrupted {
                    format!(
                        "Command '{}' was interrupted: {}.",
                        command_str, interrupt_reason,
                    )
                } else {
                    if success {
                        format!("Command '{}' finished with success", command_str)
                    } else {
                        format!("Command '{}' finished with error", command_str)
                    }
                };

                // Print status message
                if !silent_mode {
                    // Ensure we're in normal mode for console output
                    let _ = disable_raw_mode();
                    
                    if was_interrupted {
                        println!("{}🐚 Shell:{} {} (interrupted: {})",
                            FORMAT_BOLD, FORMAT_RESET, command_str, interrupt_reason);
                    } else if success {
                        println!("{}🐚 Shell:{} {} (completed successfully)", 
                            FORMAT_BOLD, FORMAT_RESET, command_str);
                    } else {
                        println!("{}🐚 Shell:{} {} (completed with error)",
                            FORMAT_BOLD, FORMAT_RESET, command_str);
                    }
                }

                // Send final completion message with result
                let _ = sender.send(ShellOutput::Complete(ToolResult {
                    success,
                    agent_output,
                }));
            }
            Err(e) => {
                // Failed to start command
                let error_msg = format!("Failed to execute command '{}': {}", command_str, e);
                
                // Log error
                if !silent_mode {
                    // Ensure terminal is in normal mode
                    let _ = disable_raw_mode();
                    
                    println!("{}🐚 Shell:{} {} (failed to start: {})",
                        FORMAT_BOLD, FORMAT_RESET, command_str, e);
                }
                
                // Send error completion
                let _ = sender.send(ShellOutput::Complete(ToolResult {
                    success: false,
                    agent_output: error_msg,
                }));
            }
        }
    });
    
    // Return the receiver for streaming output
    receiver
}