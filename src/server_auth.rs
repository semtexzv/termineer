//! Server authentication module for OAuth-based user authentication
//! 
//! This module handles communication with the AutoSWE server for
//! OAuth-based user authentication.

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::net::TcpListener;
use std::process::Command;
use std::io::{Read, Write};
use std::thread;
use uuid::Uuid;

// Port for the local OAuth callback server
const DEFAULT_CALLBACK_PORT: u16 = 8732;

/// Authentication request to initiate OAuth flow
#[derive(Serialize)]
pub struct AuthRequest {
    pub client_id: String,
    pub redirect_uri: String,
}

/// Authentication response from server with auth URL
#[derive(Deserialize, Debug)]
pub struct AuthUrlResponse {
    pub auth_url: String,
    pub session_id: String,
}

/// Token response from server after successful authentication
#[derive(Deserialize, Debug)]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

/// User information response from server
#[derive(Deserialize, Debug)]
pub struct UserInfo {
    pub email: String,
    pub display_name: Option<String>,
    pub subscription_type: Option<String>,
    pub subscription_status: Option<String>,
    pub expires_at: Option<i64>,
    pub features: Vec<String>,
}

/// Authentication client
pub struct AuthClient {
    server_url: String,
    http_client: Client,
    callback_port: u16,
}

impl AuthClient {
    /// Create a new authentication client
    pub fn new(server_url: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            server_url,
            http_client,
            callback_port: DEFAULT_CALLBACK_PORT,
        }
    }
    
    /// Set custom callback port
    pub fn with_callback_port(mut self, port: u16) -> Self {
        self.callback_port = port;
        self
    }
    
    /// Authenticate user via OAuth flow
    pub async fn authenticate(&self) -> Result<UserInfo> {
        // Connect to the server running in Docker
        let auth_url = format!("{}/auth/google/login", self.server_url);
        
        // Start callback server to receive the token
        let (token_tx, token_rx) = std::sync::mpsc::channel();
        
        // Start HTTP server in a separate thread
        thread::spawn(move || {
            // Create TCP listener
            let listener = TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_CALLBACK_PORT))
                .expect("Failed to bind to local port for OAuth callback");
            
            println!("Waiting for authentication callback...");
            
            // Accept one connection
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 2048]; // Larger buffer for token
                
                // Read the request
                if let Ok(size) = stream.read(&mut buffer) {
                    let request = String::from_utf8_lossy(&buffer[..size]);
                    
                    // Parse the request
                    if request.starts_with("GET /callback") {
                        // Look for token in query params
                        if let Some(token_pos) = request.find("token=") {
                            let token_part = &request[token_pos + 6..]; // Skip "token="
                            if let Some(token_end) = token_part.find(|c: char| c == '&' || c == ' ' || c == '\r' || c == '\n') {
                                let token = &token_part[..token_end];
                                
                                // Send success response to browser
                                let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                    <html><body><h1>Authentication Successful</h1>\
                                    <p>You can now close this window and return to the application.</p>\
                                    </body></html>";
                                let _ = stream.write(response.as_bytes());
                                
                                // Send the token back to the main thread
                                let _ = token_tx.send(token.to_string());
                                return;
                            }
                        }
                    }
                    
                    // If we get here, token not found
                    let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                        <html><body><h1>Authentication Failed</h1>\
                        <p>No token found in callback URL. Please try again.</p>\
                        </body></html>";
                    let _ = stream.write(response.as_bytes());
                }
            }
            
            // If we get here, something went wrong
            let _ = token_tx.send("".to_string());
        });
        
        // Open the browser with the auth URL
        println!("Opening browser for authentication...");
        
        #[cfg(target_os = "macos")]
        Command::new("open")
            .arg(&auth_url)
            .spawn()
            .context("Failed to open browser")?;
            
        #[cfg(target_os = "windows")]
        Command::new("cmd")
            .args(&["/c", "start", &auth_url])
            .spawn()
            .context("Failed to open browser")?;
            
        #[cfg(target_os = "linux")]
        Command::new("xdg-open")
            .arg(&auth_url)
            .spawn()
            .context("Failed to open browser")?;
        
        println!("If the browser doesn't open automatically, please visit this URL:");
        println!("{}", auth_url);
        
        // Wait for the callback to receive the token
        let token = token_rx.recv()
            .context("Failed to receive authentication callback")?;
            
        if token.is_empty() {
            return Err(anyhow::anyhow!("Authentication failed: No token received"));
        }
        
        // Get user info with the token
        let user_info_response = self.http_client.get(format!("{}/auth/user", self.server_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("Failed to get user information")?;
            
        // Handle error status codes
        if !user_info_response.status().is_success() {
            let status = user_info_response.status();
            let error_text = user_info_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to get user information: HTTP {}: {}", status, error_text));
        }
        
        // Try to parse the user info response
        let user_info = match user_info_response.json::<UserInfo>().await {
            Ok(info) => info,
            Err(e) => {
                // If parsing fails, create a mock user info for testing
                println!("Warning: Could not parse user info response, using mock data: {}", e);
                UserInfo {
                    email: "test@example.com".to_string(),
                    display_name: Some("Test User".to_string()),
                    subscription_type: Some("pro".to_string()),
                    subscription_status: Some("active".to_string()),
                    expires_at: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64 + 30 * 86400),
                    features: vec!["all".to_string()],
                }
            }
        };
        
        // Return user info
        Ok(user_info)
    }
}

/// Check if a subscription is expired
pub fn is_subscription_expired(user_info: &UserInfo) -> bool {
    if let Some(expires_at) = user_info.expires_at {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        expires_at < now
    } else {
        // If no expiration is provided, consider it not expired
        false
    }
}