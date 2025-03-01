use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use std::thread;

use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;

/// Message types for asynchronous tool execution
pub enum ToolMessage {
    /// A line of output from the tool
    Line(String),
    /// Completion signal with final result
    Complete(ToolResult),
}

/// Executes a shell command with streaming output that can be interrupted
pub fn execute_shell_async(command_to_run: String, sender: Sender<ToolMessage>, interrupt_flag: Arc<Mutex<bool>>, silent_mode: bool) {
    let shell = if cfg!(target_os = "windows") { "cmd" } else { "sh" };
    let shell_arg = if cfg!(target_os = "windows") { "/C" } else { "-c" };

    // For progress reporting only
    if !silent_mode {
        println!("{}🐚 Shell:{} {} (streaming - can be interrupted)", 
                FORMAT_BOLD, FORMAT_RESET, command_to_run);
    }

    // Use spawn instead of output to get a handle to the running process
    let command_result = Command::new(shell)
        .arg(shell_arg)
        .arg(&command_to_run)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    match command_result {
        Ok(mut child) => {
            // Shared buffers for collecting stdout and stderr
            let stdout_buffer = Arc::new(Mutex::new(String::new()));
            let stderr_buffer = Arc::new(Mutex::new(String::new()));
            
            // Take the stdout and stderr handles from the child process
            let stdout = child.stdout.take().expect("Failed to capture stdout");
            let stderr = child.stderr.take().expect("Failed to capture stderr");
            
            // Clone arc references for thread use
            let stdout_buf_clone = Arc::clone(&stdout_buffer);
            let stderr_buf_clone = Arc::clone(&stderr_buffer);
            
            // Status tracking 
            let command_running = Arc::new(Mutex::new(true));
            let stdout_running_clone = Arc::clone(&command_running);
            let stderr_running_clone = Arc::clone(&command_running);
            
            // Use the passed interrupt flag
            let interrupted_clone = Arc::clone(&interrupt_flag);
            
            // Clone sender for threads
            let stdout_sender = sender.clone();
            
            // Thread for reading stdout
            let stdout_thread = thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        // Don't print anything in silent mode
                        if !silent_mode {
                            print!("{}{}{}\r\n", FORMAT_GRAY, line, FORMAT_RESET);
                            std::io::stdout().flush().unwrap_or(());
                        }
                        
                        // Send line to the receiver for streaming to LLM
                        let _ = stdout_sender.send(ToolMessage::Line(format!("STDOUT: {}", line)));
                        
                        // Store in buffer for later processing
                        let mut buffer = stdout_buf_clone.lock().unwrap();
                        buffer.push_str(&line);
                        buffer.push('\n');
                    }
                    
                    // Check if command is still running
                    if !*stdout_running_clone.lock().unwrap() {
                        break;
                    }
                }
            });
            
            // Clone sender for stderr thread
            let stderr_sender = sender.clone();
            
            // Thread for reading stderr
            let stderr_thread = thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        // Print stderr in gray too (merged with stdout), only if not in silent mode
                        if !silent_mode {
                            print!("{}{}{}\r\n", FORMAT_GRAY, line, FORMAT_RESET);
                            std::io::stdout().flush().unwrap_or(());
                        }
                        
                        // Send line to the receiver for streaming to LLM
                        let _ = stderr_sender.send(ToolMessage::Line(format!("STDERR: {}", line)));
                        
                        // Store in buffer for later processing
                        let mut buffer = stderr_buf_clone.lock().unwrap();
                        buffer.push_str(&line);
                        buffer.push('\n');
                    }
                    
                    // Check if command is still running
                    if !*stderr_running_clone.lock().unwrap() {
                        break;
                    }
                }
            });
            
            // Main monitoring loop - will end when process ends or is interrupted
            // We don't use raw mode here because the LLM will interrupt, not the user
            loop {
                // Check if process has completed on its own
                match child.try_wait() {
                    Ok(Some(status)) => {
                        // Process has exited naturally
                        *command_running.lock().unwrap() = false;
                        
                        // Wait for stdout/stderr threads to finish processing
                        let _ = stdout_thread.join();
                        let _ = stderr_thread.join();
                        
                        // Get final outputs from shared buffers
                        let stdout = stdout_buffer.lock().unwrap().clone();
                        let stderr = stderr_buffer.lock().unwrap().clone(); 
                        
                        // Format the output in a consistent style
                        let stdout_line_count = stdout.lines().count();
                        let stderr_line_count = stderr.lines().count();
                        
                        // Combined output for agent with clear separation
                        let agent_output = format!(
                            "STDOUT (lines: {})\n{}\nSTDERR (lines: {})\n{}\n",
                            stdout_line_count, stdout, 
                            stderr_line_count, stderr
                        );
                        
                        // Send completion message
                        let success = status.success();
                        if success {
                            if !silent_mode {
                                println!("{}🐚 Shell:{} {} (completed successfully)", 
                                    FORMAT_BOLD, FORMAT_RESET, command_to_run);
                            }
                            let _ = sender.send(ToolMessage::Complete(ToolResult {
                                success: true,
                                agent_output,
                            }));
                        } else {
                            if !silent_mode {
                                println!("{}🐚 Shell:{} {} (completed with error)", 
                                    FORMAT_BOLD, FORMAT_RESET, command_to_run);
                            }
                            let _ = sender.send(ToolMessage::Complete(ToolResult {
                                success: false,
                                agent_output: format!("Error executing command '{}':\n{}", command_to_run, agent_output),
                            }));
                        }
                        break;
                    }
                    Ok(None) => {
                        // Process still running, check for interruption - add debugging
                        let is_interrupted = {
                            let flag = interrupted_clone.lock().unwrap();
                            *flag
                        };
                        
                        if is_interrupted {
                            // Add debug output
                            if !silent_mode {
                                println!("🔴 Shell process received interrupt flag, killing process...");
                            }
                            
                            // Kill the process
                            let _ = child.kill();
                            *command_running.lock().unwrap() = false;
                            
                            // Wait for stdout/stderr threads to finish processing
                            let _ = stdout_thread.join();
                            let _ = stderr_thread.join();
                            
                            // Get current outputs from shared buffers
                            let stdout = stdout_buffer.lock().unwrap().clone();
                            let stderr = stderr_buffer.lock().unwrap().clone(); 
                            
                            let stdout_line_count = stdout.lines().count();
                            let stderr_line_count = stderr.lines().count();
                            
                            // Combined output for agent with clear labels
                            let agent_output = format!(
                                "Command '{}' was interrupted by LLM.\nPartial output:\nSTDOUT (lines: {})\n{}\nSTDERR (lines: {})\n{}\n",
                                command_to_run, stdout_line_count, stdout, stderr_line_count, stderr
                            );
                            
                            // Send completion message - interruption is a SUCCESS, not an error
                            if !silent_mode {
                                println!("{}🐚 Shell:{} {} (interrupted by LLM)",
                                    FORMAT_BOLD, FORMAT_RESET, command_to_run);
                            }
                            let _ = sender.send(ToolMessage::Complete(ToolResult {
                                success: true,  // Interruption is a successful outcome
                                agent_output,
                            }));
                            break;
                        }
                    }
                    Err(e) => {
                        // Error checking process status
                        *command_running.lock().unwrap() = false;
                        
                        // Send error message
                        let error_msg = format!("Error monitoring process status: {}", e);
                        if !silent_mode {
                            println!("{}🐚 Shell:{} {} (error: {})",
                                FORMAT_BOLD, FORMAT_RESET, command_to_run, e);
                        }
                        let _ = sender.send(ToolMessage::Complete(ToolResult {
                            success: false,
                            agent_output: error_msg,
                        }));
                        break;
                    }
                }
                
                // Smaller delay to be more responsive to keyboard interrupts while avoiding CPU spin
                thread::sleep(std::time::Duration::from_millis(10));
            }
        },
        Err(e) => {
            // Failed to start the command
            let agent_output = format!("Failed to execute command '{}': {}", command_to_run, e);
            
            // Print output directly if not in silent mode
            if !silent_mode {
                println!("{}🐚 Shell:{} {} (failed to start: {})",
                    FORMAT_BOLD, FORMAT_RESET,
                    command_to_run,
                    e
                );
            }
            
            // Send completion message
            let _ = sender.send(ToolMessage::Complete(ToolResult {
                success: false,
                agent_output,
            }));
        },
    }
}