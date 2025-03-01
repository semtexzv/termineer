use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal;
use crate::tools::ToolResult;
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};

// No scrolling display - just stream output directly

pub fn execute_shell(args: &str, body: &str, silent_mode: bool) -> ToolResult {
    // If body is provided, use it as a script instead of the args
    let command_to_run = if !body.is_empty() {
        body
    } else {
        args
    };
    let shell = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };

    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    // Use spawn instead of output to get a handle to the running process
    let command_result = Command::new(shell)
        .arg(shell_arg)
        .arg(command_to_run)
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
            
            // Setup for handling interrupts
            let mut interrupted = false;
            
            // Print status message with consistent formatting only if not in silent mode
            if !silent_mode {
                // Print status message with consistent bold formatting
                println!("{}🐚 Shell:{} {} (Press Ctrl+C to interrupt)", 
                        FORMAT_BOLD, FORMAT_RESET, args);
            }

            // We want to enable raw mode to capture Ctrl+C, but we need to ensure
            // we restore terminal state properly regardless of how we exit
            let raw_mode_result = terminal::enable_raw_mode();
            let raw_mode_enabled = raw_mode_result.is_ok();
            
            // Poll for keyboard events while checking process status
            loop {
                // Check if process has completed on its own
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        // Process has exited
                        *command_running.lock().unwrap() = false;
                        break;
                    }
                    Ok(None) => {
                        // Process still running, continue polling
                    }
                    Err(e) => {
                        // Error checking process status
                        eprintln!("Error checking process status: {}", e);
                        *command_running.lock().unwrap() = false;
                        break;
                    }
                }
                
                // Check for keyboard input (with short timeout to not block)
                if raw_mode_enabled && crossterm::event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                            interrupted = true;
                            // Mark as not running and kill the process
                            *command_running.lock().unwrap() = false;
                            let _ = child.kill();
                            break;
                        }
                    }
                }
                
                // No progress indicator for cleaner output
            }
            
            // Restore terminal state if we modified it
            if raw_mode_enabled {
                let _ = terminal::disable_raw_mode();
                // Raw mode is now disabled
            }
            
            // Wait for stdout/stderr threads to finish processing
            let _ = stdout_thread.join();
            let _ = stderr_thread.join();
            
            // Get final outputs from shared buffers
            let stdout = stdout_buffer.lock().unwrap().clone();
            let stderr = stderr_buffer.lock().unwrap().clone(); 
            
            // Get exit status if process wasn't interrupted
            let success = if interrupted {
                false
            } else {
                match child.try_wait() {
                    Ok(Some(status)) => status.success(),
                    _ => false
                }
            };
            
            // Format the output in a consistent style with other tools
            if success {
                // Count lines for output
                let stdout_line_count = stdout.lines().count();
                let stderr_line_count = stderr.lines().count();
                let _total_lines = stdout_line_count + stderr_line_count;
                
                // Combined output for agent with clear separation
                let agent_output = format!(
                    "STDOUT (lines: {})\n{}\nSTDERR (lines: {})\n{}\n",
                    stdout_line_count, stdout, 
                    stderr_line_count, stderr
                );
                
                // Print output directly if not in silent mode
                if !silent_mode {
                    println!("{}🐚 Shell:{} {} (success)", 
                        FORMAT_BOLD, FORMAT_RESET, args);
                    // Note: Not printing output summary here as it was already streamed in real-time
                }
                
                ToolResult {
                    success: true,
                    agent_output,
                }
            } else if interrupted {
                // Command was interrupted by user
                let stdout_line_count = stdout.lines().count();
                let stderr_line_count = stderr.lines().count();
                
                // Combined output for agent with clear labels
                let agent_output = format!(
                    "Command '{}' was interrupted by user.\nPartial output:\nSTDOUT (lines: {})\n{}\nSTDERR (lines: {})\n{}\n",
                    args, stdout_line_count, stdout, stderr_line_count, stderr
                );
                
                // Print output directly if not in silent mode
                if !silent_mode {
                    println!("{}🐚 Shell:{} {} (interrupted by user)",
                        FORMAT_BOLD, FORMAT_RESET, args);
                }
                
                ToolResult {
                    success: false,
                    agent_output,
                }
            } else {
                // Command failed with error
                let stdout_line_count = stdout.lines().count();
                let stderr_line_count = stderr.lines().count();
                
                // Combined full output for agent
                let agent_output = format!(
                    "Error executing command '{}':\nSTDOUT (lines: {})\n{}\nSTDERR (lines: {})\n{}\n", 
                    args, stdout_line_count, stdout, stderr_line_count, stderr
                );
                
                // Print output directly if not in silent mode
                if !silent_mode {
                    println!("{}🐚 Shell:{} {} (failed with error)",
                        FORMAT_BOLD, FORMAT_RESET, args);
                    // Note: Not printing error details here as they were already streamed in real-time
                }
                
                ToolResult {
                    success: false,
                    agent_output,
                }
            }
        },
        Err(e) => {
            // Failed to start the command
            let agent_output = format!("Failed to execute command '{}': {}", args, e);
            
            // Print output directly if not in silent mode
            if !silent_mode {
                println!("{}🐚 Shell:{} {} (failed to start: {})",
                    FORMAT_BOLD, FORMAT_RESET,
                    args,
                    e
                );
            }
            
            ToolResult {
                success: false,
                agent_output,
            }
        },
    }
}