//! Process-based connection handler for MCP client

use async_trait::async_trait;
use futures::TryFutureExt;
use std::future::ready;
use std::process::Command;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::AsyncBufReadExt;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

use crate::mcp::error::{McpError, McpResult};
use crate::mcp::protocol::JsonRpcMessage;
use crate::mcp::Connection;
use crate::output;

macro_rules! async_writeln {
    ($dst: expr) => {
        {
            tokio::io::AsyncWriteExt::write_all(&mut $dst, b"\n").await
        }
    };
    ($dst: expr, $fmt: expr) => {
        {
            use std::io::Write;
            let mut buf = Vec::<u8>::new();
            writeln!(buf, $fmt).unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut $dst, &buf).await
        }
    };
    ($dst: expr, $fmt: expr, $($arg: tt)*) => {
        {
            use std::io::Write;
            let mut buf = Vec::<u8>::new();
            writeln!(buf, $fmt, $( $arg )*).unwrap();
            tokio::io::AsyncWriteExt::write_all(&mut $dst, &buf).await
        }
    };
}

/// Command messages for the connection manager
enum ConnectionCommand {
    /// Send a message to the server
    SendMessage {
        message: JsonRpcMessage,
        response_sender: oneshot::Sender<McpResult<JsonRpcMessage>>,
    },
    /// Close the connection
    Close,
}

/// Process-based connection manager for MCP
pub struct ProcessConnection {
    command_sender: mpsc::Sender<ConnectionCommand>,
    connected: Arc<AtomicBool>,
    child_process: Arc<Mutex<Option<Child>>>,
}

impl ProcessConnection {
    /// Create a new process connection with the given command
    pub async fn spawn(name: &str, executable: &str, args: &[&str]) -> McpResult<Self> {
        // Start the child process
        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                bprintln !(error:"Failed to start MCP process: {}", e);
                McpError::ConnectionError(format!("Failed to start process: {}", e))
            })?;

        // Get stdin and stdout pipes
        let stdin =
            tokio::process::ChildStdin::from_std(child.stdin.take().ok_or_else(|| {
                McpError::ConnectionError("Failed to open stdin pipe".to_string())
            })?)
            .map_err(|e| McpError::ConnectionError(format!("Failed to start process: {}", e)))?;

        let stdout =
            tokio::process::ChildStdout::from_std(child.stdout.take().ok_or_else(|| {
                McpError::ConnectionError("Failed to open stdout pipe".to_string())
            })?)
            .map_err(|e| McpError::ConnectionError(format!("Failed to start process: {}", e)))?;

        let name = name.to_string();
        // Make a clone for the async closure
        let name_for_stderr = name.clone();
        
        // Capture stderr for logging MCP output to the user
        if let Some(Ok(stderr)) = child
            .stderr
            .take()
            .map(tokio::process::ChildStderr::from_std)
        {
            let stderr_reader = BufReader::new(stderr);
            output::spawn(async move {
                let mut lines = stderr_reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Display MCP output with cyan prefix including server name, but normal content
                    bprintln!(
                        "{}MCP[{}]:{} {}",
                        crate::constants::FORMAT_CYAN,
                        name_for_stderr,
                        crate::constants::FORMAT_RESET,
                        line
                    );
                }
            });
        } else {
            bprintln !(error:"Failed to capture stderr from MCP process");
        }

        // Create command channel
        let (cmd_sender, cmd_receiver) = mpsc::channel::<ConnectionCommand>(32);

        // Create connection state
        let connected = Arc::new(AtomicBool::new(true));
        let connected_clone = connected.clone();

        // Store child process
        let child_process = Arc::new(Mutex::new(Some(child)));
        let child_process_clone = child_process.clone();

        // Spawn the connection manager task
        output::spawn(Self::connection_task(
            stdin,
            stdout,
            cmd_receiver,
            connected_clone,
            child_process_clone,
        ));

        Ok(Self {
            command_sender: cmd_sender,
            connected,
            child_process,
        })
    }

    /// Send a message and wait for a response
    pub async fn send_message(&self, message: JsonRpcMessage) -> McpResult<JsonRpcMessage> {
        // Check if connected
        if !self.connected.load(Ordering::SeqCst) {
            return Err(McpError::ServerDisconnected);
        }

        // Create response channel
        let (response_sender, response_receiver) = oneshot::channel();

        // Send command to connection task
        self.command_sender
            .send(ConnectionCommand::SendMessage {
                message,
                response_sender,
            })
            .await
            .map_err(|_| McpError::ServerDisconnected)?;

        // Wait for response
        response_receiver
            .await
            .map_err(|_| McpError::ResponseNotReceived)?
    }

    /// Close the process connection
    pub async fn close(&self) -> McpResult<()> {
        if self.connected.load(Ordering::SeqCst) {
            self.command_sender
                .send(ConnectionCommand::Close)
                .await
                .map_err(|_| McpError::ServerDisconnected)?;
        }
        Ok(())
    }

    /// Returns true if the connection is active
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Connection manager task
    async fn connection_task(
        stdin: tokio::process::ChildStdin,
        stdout: tokio::process::ChildStdout,
        mut cmd_receiver: mpsc::Receiver<ConnectionCommand>,
        connected: Arc<AtomicBool>,
        child_process: Arc<Mutex<Option<Child>>>,
    ) {
        // Wrap pipes for easier use
        let mut stdin = stdin;
        let stdout = BufReader::new(stdout);

        // Response waiters indexed by message ID
        let pending_responses: std::collections::HashMap<
            String,
            oneshot::Sender<McpResult<JsonRpcMessage>>,
        > = std::collections::HashMap::new();

        // Clone pointers for the reader task
        let connected_clone = connected.clone();
        let pending_responses_clone = Arc::new(Mutex::new(pending_responses));

        // Spawn a task to read from the process's stdout
        let reader_task = {
            let pending_responses = pending_responses_clone.clone();

            output::spawn(async move {
                let mut lines = stdout.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if let Ok(rpc_msg) = serde_json::from_str::<JsonRpcMessage>(&line) {
                        // Check if this is a response to a pending request
                        if let Some(id) = &rpc_msg.id {
                            let id_str = id.to_string();

                            let mut pending = pending_responses.try_lock().unwrap();
                            if let Some(sender) = pending.remove(&id_str) {
                                sender.send(Ok(rpc_msg)).unwrap();
                            }
                        }
                    } else {
                        // Non-JSON or invalid message
                        bprintln !(error:"Received invalid JSON from MCP process: {}", line);
                    }
                }

                // EOF reached, process has terminated or closed pipe
                connected_clone.store(false, Ordering::SeqCst);
            })
        };

        // Run until closed or error
        while let Some(cmd) = cmd_receiver.recv().await {
            match cmd {
                ConnectionCommand::SendMessage {
                    message,
                    response_sender,
                } => {
                    // Serialize message
                    let text = match serde_json::to_string(&message) {
                        Ok(t) => t,
                        Err(e) => {
                            let _ = response_sender.send(Err(McpError::JsonError(e)));
                            continue;
                        }
                    };

                    // Store response sender if message has an ID
                    if let Some(id) = &message.id {
                        let id_str = id.to_string();
                        {
                            let mut pending = pending_responses_clone.try_lock().unwrap();
                            pending.insert(id_str, response_sender);
                        }

                        // Send to process and flush to ensure it's sent immediately
                        if let Err(e) = ready(async_writeln!(stdin, "{}", text))
                            .and_then(|()| stdin.flush())
                            .await
                        {
                            // Failed to write, remove from pending
                            let mut pending = pending_responses_clone.lock().unwrap();
                            if let Some(sender) = pending.remove(&id.to_string()) {
                                let _ = sender.send(Err(McpError::ConnectionError(format!(
                                    "Failed to write to process: {}",
                                    e
                                ))));
                            }

                            // Mark as disconnected
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }
                    } else {
                        // Message has no ID, so won't get a response
                        let _ = response_sender.send(Err(McpError::ProtocolError(
                            "Cannot wait for response for message with no ID".to_string(),
                        )));

                        // Still send the message and flush to ensure it's sent immediately
                        if let Err(e) = ready(async_writeln!(stdin, "{}", text))
                            .and_then(|()| stdin.flush())
                            .await
                        {
                            bprintln !(error:"Failed to write to process: {}", e);
                            connected.store(false, Ordering::SeqCst);
                            break;
                        }
                    }
                }
                ConnectionCommand::Close => {
                    // Close the connection by killing the process
                    let mut process_guard = child_process.lock().unwrap();
                    if let Some(child) = process_guard.as_mut() {
                        let _ = child.kill();
                    }
                    *process_guard = None;

                    // Mark as disconnected
                    connected.store(false, Ordering::SeqCst);
                    break;
                }
            }
        }

        // Wait for reader task to complete
        let _ = reader_task.await;

        // Clean up child process if still running
        let mut process_guard = child_process.lock().unwrap();
        if let Some(child) = process_guard.as_mut() {
            let _ = child.kill();
        }
        *process_guard = None;

        // Update connection state
        connected.store(false, Ordering::SeqCst);

        // Notify all pending responses that the connection is closed
        let mut pending = pending_responses_clone.lock().unwrap();
        for (_, sender) in pending.drain() {
            let _ = sender.send(Err(McpError::ServerDisconnected));
        }
    }
}

#[async_trait]
impl Connection for ProcessConnection {
    async fn send_message(&self, message: JsonRpcMessage) -> McpResult<JsonRpcMessage> {
        ProcessConnection::send_message(self, message).await
    }

    async fn close(&self) -> McpResult<()> {
        ProcessConnection::close(self).await
    }

    fn is_connected(&self) -> bool {
        ProcessConnection::is_connected(self)
    }
}

impl Drop for ProcessConnection {
    fn drop(&mut self) {
        // Ensure the process is terminated when the connection is dropped
        let mut process_guard = self.child_process.lock().unwrap();
        if let Some(child) = process_guard.as_mut() {
            let _ = child.kill();
        }
        *process_guard = None;
    }
}
