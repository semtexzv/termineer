use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use crate::constants::{FORMAT_BOLD, FORMAT_GRAY, FORMAT_RESET};
use crate::tools::ToolResult;

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
    silent_mode: bool,
) -> Result<mpsc::Receiver<ShellOutput>, Box<dyn std::error::Error>> {
    // If body is provided, use it as a script instead of the args
    let command_str = if !body.is_empty() {
        body.to_string()
    } else {
        command_to_run.to_string()
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

    // Create a channel for output streaming
    let (sender, receiver) = mpsc::channel(100); // Buffer size of 100 messages

    // Print initial status message
    if !silent_mode {
        // Use buffer system for output - no need to worry about terminal mode

        // Use output buffer for shell status message
        crate::btool_println!(
            "shell",
            "{}🐚 Shell:{} {} (streaming - can be interrupted)",
            FORMAT_BOLD,
            FORMAT_RESET,
            command_str
        );
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

    crate::output::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut _line_count = 0;

        while let Ok(Some(line)) = reader.next_line().await {
            // Display line if not in silent mode
            if !stdout_silent {
                // Use output buffer for shell output
                crate::btool_println!("shell", "{}{}{}", FORMAT_GRAY, line, FORMAT_RESET);
                _line_count += 1;
            }

            // Send the line through the channel
            if stdout_sender
                .send(ShellOutput::Stdout(line.clone()))
                .await
                .is_err()
            {
                break;
            }

            // Check if we should exit
            if !*stdout_running_clone.lock().await {
                break;
            }
        }

        // No need to manually flush output when using buffer system
    });

    // Stderr reader task
    let stderr_sender = sender.clone();
    let stderr_running_clone = Arc::clone(&command_running);
    let stderr_silent = silent_mode;

    crate::output::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut _line_count = 0;

        while let Ok(Some(line)) = reader.next_line().await {
            // Display line if not in silent mode
            if !stderr_silent {
                // Use output buffer for shell stderr output
                crate::btool_println!("shell", "{}{}{}", FORMAT_GRAY, line, FORMAT_RESET);
                _line_count += 1;
            }

            // Send the line through the channel
            if stderr_sender
                .send(ShellOutput::Stderr(line.clone()))
                .await
                .is_err()
            {
                break;
            }

            // Check if we should exit
            if !*stderr_running_clone.lock().await {
                break;
            }
        }

        // No need to manually flush output when using buffer system
    });

    // No longer handling keyboard interruptions directly in the shell tool
    // Interrupts now come from the UI layer through the InterruptData

    // Spawn the main monitoring task
    let main_command_str = command_str.clone();
    let main_sender = sender.clone();
    let main_silent_mode = silent_mode;

    crate::output::spawn(async move {
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
                        // Using buffer system for output - terminal modes handled elsewhere

                        // Use output buffer for error message
                        crate::btool_println!(
                            "shell",
                            "{}🐚 Shell:{} Error monitoring process: {}",
                            FORMAT_BOLD,
                            FORMAT_RESET,
                            e
                        );
                    }

                    // Send error completion
                    let _ = main_sender
                        .send(ShellOutput::Complete(ToolResult {
                            success: false,
                            agent_output: format!("Error monitoring process status: {}", e),
                        }))
                        .await;
                    return;
                }
            }

            // Brief delay to avoid CPU spinning
            sleep(Duration::from_millis(10)).await;
        }

        // Wait a moment for stdout/stderr to finish processing
        sleep(Duration::from_millis(50)).await;

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
            // Use output buffer for completion status
            if was_interrupted {
                crate::btool_println!(
                    "shell",
                    "{}🐚 Shell:{} {} (interrupted: {})",
                    FORMAT_BOLD,
                    FORMAT_RESET,
                    main_command_str,
                    interrupt_reason
                );
            } else if success {
                crate::btool_println!(
                    "shell",
                    "{}🐚 Shell:{} {} (completed successfully)",
                    FORMAT_BOLD,
                    FORMAT_RESET,
                    main_command_str
                );
            } else {
                crate::btool_println!(
                    "shell",
                    "{}🐚 Shell:{} {} (completed with error)",
                    FORMAT_BOLD,
                    FORMAT_RESET,
                    main_command_str
                );
            }
        }

        // Send final completion message with result
        let _ = main_sender
            .send(ShellOutput::Complete(ToolResult {
                success,
                agent_output,
            }))
            .await;
    });

    // Return the receiver for streaming output
    Ok(receiver)
}
