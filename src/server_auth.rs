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
        // Generate a unique client ID
        let client_id = format!("client_{}", Uuid::new_v4().to_string());
        
        // Redirect URI for the local callback server
        let redirect_uri = format!("http://localhost:{}/callback", self.callback_port);
        
        // Create request payload
        let request = AuthRequest {
            client_id: client_id.clone(),
            redirect_uri: redirect_uri.clone(),
        };
        
        // Send request to server to get authentication URL
        let response = self.http_client.post(format!("{}/auth/init", self.server_url))
            .json(&request)
            .send()
            .await
            .context("Failed to initiate authentication")?;
            
        // Handle error status codes
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Authentication initialization failed: HTTP {}: {}", status, error_text));
        }
        
        // Parse response to get auth URL
        let auth_url_response = response.json::<AuthUrlResponse>().await
            .context("Failed to parse authentication initialization response")?;
        
        // Start local HTTP server to receive callback
        let (auth_code_tx, auth_code_rx) = std::sync::mpsc::channel();
        
        let session_id = auth_url_response.session_id.clone();
        
        // Start HTTP server in a separate thread
        thread::spawn(move || {
            // Create TCP listener
            let listener = TcpListener::bind(format!("127.0.0.1:{}", DEFAULT_CALLBACK_PORT))
                .expect("Failed to bind to local port for OAuth callback");
            
            println!("Waiting for authentication callback...");
            
            // Accept one connection
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 1024];
                
                // Read the request
                if let Ok(size) = stream.read(&mut buffer) {
                    let request = String::from_utf8_lossy(&buffer[..size]);
                    
                    // Parse the request
                    if request.starts_with("GET /callback") {
                        // Extract code parameter from URL if present
                        if let Some(query_start) = request.find('?') {
                            if let Some(query_end) = request[query_start..].find(' ') {
                                // Extract query parameter ignored since we only need the session ID
                                let _query = &request[query_start + 1..query_start + query_end];
                                
                                // Send success response to browser
                                let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                    <html><body><h1>Authentication Successful</h1>\
                                    <p>You can now close this window and return to the application.</p>\
                                    </body></html>";
                                let _ = stream.write(response.as_bytes());
                                
                                // Send the session ID back to the main thread
                                let _ = auth_code_tx.send(session_id.clone());
                                return;
                            }
                        }
                    }
                    
                    // If we get here, something went wrong
                    // Send error response to browser
                    let response = "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                        <html><body><h1>Authentication Failed</h1>\
                        <p>Please try again.</p>\
                        </body></html>";
                    let _ = stream.write(response.as_bytes());
                }
            }
            
            // If we get here, something went wrong
            let _ = auth_code_tx.send("".to_string());
        });
        
        // Open the browser with the auth URL
        println!("Opening browser for authentication...");
        
        #[cfg(target_os = "macos")]
        Command::new("open")
            .arg(&auth_url_response.auth_url)
            .spawn()
            .context("Failed to open browser")?;
            
        #[cfg(target_os = "windows")]
        Command::new("cmd")
            .args(&["/c", "start", &auth_url_response.auth_url])
            .spawn()
            .context("Failed to open browser")?;
            
        #[cfg(target_os = "linux")]
        Command::new("xdg-open")
            .arg(&auth_url_response.auth_url)
            .spawn()
            .context("Failed to open browser")?;
        
        println!("If the browser doesn't open automatically, please visit this URL:");
        println!("{}", auth_url_response.auth_url);
        
        // Wait for the callback to receive the auth code
        let session_id = auth_code_rx.recv()
            .context("Failed to receive authentication callback")?;
            
        if session_id.is_empty() {
            return Err(anyhow::anyhow!("Authentication failed: No session ID received"));
        }
        
        // Exchange session ID for tokens
        let token_response = self.http_client.post(format!("{}/auth/token", self.server_url))
            .json(&serde_json::json!({
                "session_id": session_id,
                "client_id": client_id,
            }))
            .send()
            .await
            .context("Failed to exchange session ID for tokens")?;
            
        // Handle error status codes
        if !token_response.status().is_success() {
            let status = token_response.status();
            let error_text = token_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Token exchange failed: HTTP {}: {}", status, error_text));
        }
        
        // Parse token response
        let token_info = token_response.json::<AuthTokenResponse>().await
            .context("Failed to parse token response")?;
            
        // Get user info with the access token
        let user_info_response = self.http_client.get(format!("{}/auth/user", self.server_url))
            .header("Authorization", format!("Bearer {}", token_info.access_token))
            .send()
            .await
            .context("Failed to get user information")?;
            
        // Handle error status codes
        if !user_info_response.status().is_success() {
            let status = user_info_response.status();
            let error_text = user_info_response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("Failed to get user information: HTTP {}: {}", status, error_text));
        }
        
        // Parse user info response
        let user_info = user_info_response.json::<UserInfo>().await
            .context("Failed to parse user information response")?;
            
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