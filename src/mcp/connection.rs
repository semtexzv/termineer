//! WebSocket connection handler for MCP client

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::connect_async;
use futures::{stream::{SplitSink, SplitStream}, SinkExt, StreamExt};
use url::Url;
use async_trait::async_trait;

use crate::mcp::error::{McpError, McpResult};
use crate::mcp::protocol::JsonRpcMessage;
use crate::mcp::Connection;

#[allow(dead_code)]
type WebSocketSender = SplitSink<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>;
#[allow(dead_code)]
type WebSocketReceiver = SplitStream<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;

/// Command messages for the connection manager
enum Command {
    /// Send a message to the server
    SendMessage {
        #[allow(dead_code)]
        message: JsonRpcMessage,
        #[allow(dead_code)]
        response_sender: oneshot::Sender<McpResult<JsonRpcMessage>>,
    },
    /// Close the connection
    Close,
}

/// WebSocket connection manager
pub struct WebSocketConnection {
    command_sender: mpsc::Sender<Command>,
    connected: Arc<AtomicBool>,
}

impl WebSocketConnection {
    /// Create a new WebSocket connection to the given URL
    #[allow(dead_code)]
    pub async fn connect(url_str: &str) -> McpResult<Self> {
        // Parse URL
        let url = Url::parse(url_str)?;
        
        // Connect to WebSocket server
        let (ws_stream, _) = connect_async(url).await
            .map_err(|e| McpError::ConnectionError(format!("Failed to connect: {}", e)))?;
        
        // Split the WebSocket stream
        let (ws_sender, ws_receiver) = ws_stream.split();
        
        // Create command channel
        let (cmd_sender, cmd_receiver) = mpsc::channel(32);
        
        // Create connection state
        let connected = Arc::new(AtomicBool::new(true));
        let connected_clone = connected.clone();
        
        // Spawn the connection manager task
        tokio::spawn(Self::connection_task(
            ws_sender,
            ws_receiver,
            cmd_receiver,
            connected_clone,
        ));
        
        Ok(Self {
            command_sender: cmd_sender,
            connected,
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
        self.command_sender.send(Command::SendMessage {
            message,
            response_sender,
        }).await
        .map_err(|_| McpError::ServerDisconnected)?;
        
        // Wait for response
        response_receiver.await
            .map_err(|_| McpError::ResponseNotReceived)?
    }
    
    /// Close the WebSocket connection
    pub async fn close(&self) -> McpResult<()> {
        if self.connected.load(Ordering::SeqCst) {
            self.command_sender.send(Command::Close).await
                .map_err(|_| McpError::ServerDisconnected)?;
        }
        Ok(())
    }
    
    /// Returns true if the connection is active
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
    
    /// Connection manager task
    #[allow(dead_code)]
    async fn connection_task(
        mut ws_sender: WebSocketSender,
        mut ws_receiver: WebSocketReceiver,
        mut cmd_receiver: mpsc::Receiver<Command>,
        connected: Arc<AtomicBool>,
    ) {
        // Response waiters indexed by message ID
        let mut pending_responses = std::collections::HashMap::new();
        
        // Run until closed or error
        loop {
            tokio::select! {
                // Handle incoming WebSocket messages
                ws_msg = ws_receiver.next() => {
                    match ws_msg {
                        Some(Ok(msg)) => {
                            if let Err(e) = Self::handle_ws_message(msg, &mut pending_responses).await {
                                eprintln!("Error handling WebSocket message: {}", e);
                                break;
                            }
                        },
                        Some(Err(e)) => {
                            eprintln!("WebSocket error: {}", e);
                            break;
                        },
                        None => {
                            // WebSocket closed
                            break;
                        }
                    }
                },
                
                // Handle commands from the client
                cmd = cmd_receiver.recv() => {
                    match cmd {
                        Some(Command::SendMessage { message, response_sender }) => {
                            if let Err(e) = Self::handle_send_command(
                                &mut ws_sender, 
                                message, 
                                response_sender, 
                                &mut pending_responses
                            ).await {
                                eprintln!("Error sending message: {}", e);
                                break;
                            }
                        },
                        Some(Command::Close) => {
                            // Close the connection
                            if let Err(e) = ws_sender.close().await {
                                eprintln!("Error closing WebSocket: {}", e);
                            }
                            break;
                        },
                        None => {
                            // Command channel closed
                            break;
                        }
                    }
                }
            }
        }
        
        // Update connection state
        connected.store(false, Ordering::SeqCst);
        
        // Notify all pending responses that the connection is closed
        for (_, sender) in pending_responses {
            let _ = sender.send(Err(McpError::ServerDisconnected));
        }
    }
    
    /// Handle an incoming WebSocket message
    #[allow(dead_code)]
    async fn handle_ws_message(
        msg: WsMessage,
        pending_responses: &mut std::collections::HashMap<String, oneshot::Sender<McpResult<JsonRpcMessage>>>,
    ) -> McpResult<()> {
        match msg {
            WsMessage::Text(text) => {
                // Parse JSON-RPC message
                let rpc_msg: JsonRpcMessage = serde_json::from_str(&text)?;
                
                // Check if this is a response to a pending request
                if let Some(id) = &rpc_msg.id {
                    let id_str = id.to_string();
                    if let Some(sender) = pending_responses.remove(&id_str) {
                        let _ = sender.send(Ok(rpc_msg));
                    }
                }
                Ok(())
            },
            WsMessage::Close(_) => {
                Err(McpError::ServerDisconnected)
            },
            _ => Ok(()) // Ignore other message types
        }
    }
    
    /// Handle a send message command
    #[allow(dead_code)]
    async fn handle_send_command(
        ws_sender: &mut WebSocketSender,
        message: JsonRpcMessage,
        response_sender: oneshot::Sender<McpResult<JsonRpcMessage>>,
        pending_responses: &mut std::collections::HashMap<String, oneshot::Sender<McpResult<JsonRpcMessage>>>,
    ) -> McpResult<()> {
        // Serialize message
        let text = serde_json::to_string(&message)?;
        
        // Store response sender if message has an ID
        if let Some(id) = &message.id {
            let id_str = id.to_string();
            pending_responses.insert(id_str, response_sender);
            
            // Send WebSocket message
            ws_sender.send(WsMessage::Text(text)).await
                .map_err(|e| McpError::WebSocketError(e))?;
        } else {
            // Message has no ID, so won't get a response
            let _ = response_sender.send(Err(McpError::ProtocolError(
                "Cannot wait for response for message with no ID".to_string()
            )));
            
            // Still send the message
            ws_sender.send(WsMessage::Text(text)).await
                .map_err(|e| McpError::WebSocketError(e))?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl Connection for WebSocketConnection {
    // Implement the trait methods by calling the struct's own methods
    // This avoids recursion since they have different signatures
    async fn send_message(&self, message: JsonRpcMessage) -> McpResult<JsonRpcMessage> {
        // Use the WebSocketConnection's implementation of send_message directly
        WebSocketConnection::send_message(self, message).await
    }
    
    async fn close(&self) -> McpResult<()> {
        // Use the WebSocketConnection's implementation of close directly
        WebSocketConnection::close(self).await
    }
    
    fn is_connected(&self) -> bool {
        // Use the WebSocketConnection's implementation of is_connected directly
        WebSocketConnection::is_connected(self)
    }
}

impl Drop for WebSocketConnection {
    fn drop(&mut self) {
        if self.connected.load(Ordering::SeqCst) {
            eprintln!("WebSocketConnection dropped while still connected");
        }
    }
}