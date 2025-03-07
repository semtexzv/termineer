use axum::http::StatusCode;
use axum::{
    extract::{Json, Query},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;

// Configuration constants
const LOCAL_PORT: u16 = 3030;

/// User information returned from the server
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UserInfo {
    pub email: String,
    pub display_name: Option<String>,
    pub subscription_type: Option<String>,
    pub subscription_status: Option<String>,
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub features: Vec<String>,
}

/// Authentication response containing the JWT token and user information
#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

// Application state shared between request handlers
struct AppState {
    token_tx: Mutex<Option<oneshot::Sender<AuthResponse>>>,
}

// Request structure for POST auth data
#[derive(Debug, Deserialize)]
struct AuthDataRequest {
    data: String,
}

// Response structure for POST auth data
#[derive(Debug, Serialize)]
struct AuthDataResponse {
    success: bool,
    message: String,
}

/// Authentication client for handling OAuth flows with the server
pub struct AuthClient {
    server_url: String,
    http_client: reqwest::Client,
}

impl AuthClient {
    /// Create a new authentication client
    pub fn new(server_url: String) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            server_url,
            http_client,
        }
    }

    /// Get user information using cached credentials
    pub async fn get_cached_user_info(&self) -> Result<UserInfo, anyhow::Error> {
        // Load the saved token
        let token = load_token().ok_or_else(|| anyhow::anyhow!("No saved token found"))?;
        println!("Successfully loaded token, now verifying with server");

        // Try to get user info with the current token
        match self.get_user_info_with_token(&token).await {
            Ok(user_info) => {
                // Token is valid, return user info
                Ok(user_info)
            }
            Err(e) => {
                println!("Error with current token: {}", e);

                // Check if this looks like a server configuration error
                // Handle both the old and new error message formats
                if e.to_string().contains("Server configuration error")
                    || e.to_string().contains("AppState not found")
                    || e.to_string()
                        .contains("Authentication middleware not properly configured")
                {
                    println!("Detected server configuration error - this might be temporary");
                    println!("Error details: {}", e);
                    println!("Attempting token refresh to resolve the issue...");

                    // Let's try a token refresh
                    match self.refresh_token(&token).await {
                        Ok(new_token) => {
                            println!("Successfully refreshed token, trying again");
                            // Save the new token
                            save_token(&new_token)?;

                            // Try again with the new token
                            self.get_user_info_with_token(&new_token).await
                        }
                        Err(refresh_err) => {
                            println!("Failed to refresh token: {}", refresh_err);
                            // Return original error
                            Err(e)
                        }
                    }
                } else {
                    // For other errors, just return the original error
                    Err(e)
                }
            }
        }
    }

    /// Get user info with a specific token
    async fn get_user_info_with_token(&self, token: &str) -> Result<UserInfo, anyhow::Error> {
        // Get user info from the server
        println!("Sending request to: {}/auth/user", self.server_url);
        let user_info_response = self
            .http_client
            .get(format!("{}/auth/user", self.server_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        println!("Server response status: {}", user_info_response.status());

        // Handle error status codes
        if !user_info_response.status().is_success() {
            let status = user_info_response.status();
            println!("Error response received with status: {}", status);

            let error_text = user_info_response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            println!("Error response body: {}", error_text);

            return Err(anyhow::anyhow!(
                "Failed to get user information: HTTP {}: {}",
                status,
                error_text
            ));
        }

        // Get response body for debugging
        let response_text = user_info_response.text().await?;
        println!("Successful response body: {}", response_text);

        // Parse the response
        let user_info = match serde_json::from_str::<UserInfo>(&response_text) {
            Ok(info) => {
                println!("Successfully parsed user info for: {}", info.email);
                info
            }
            Err(e) => {
                println!("Error parsing user info JSON: {}", e);
                return Err(anyhow::anyhow!("Failed to parse user info: {}", e));
            }
        };

        Ok(user_info)
    }

    /// Attempt to refresh a token
    async fn refresh_token(&self, old_token: &str) -> Result<String, anyhow::Error> {
        println!("Attempting to refresh token");

        // Since we don't have a dedicated refresh endpoint, we'll attempt
        // to re-authenticate through the browser flow
        println!("Re-authentication required - will need browser-based login");
        let user_info = self.authenticate().await?;

        // We don't need to return the token since authenticate() already saved it
        // But we'll load it to verify it was saved properly
        match load_token() {
            Some(token) => Ok(token),
            None => Err(anyhow::anyhow!("Failed to save refreshed token")),
        }
    }

    /// Authenticate the user through the browser-based flow
    pub async fn authenticate(&self) -> Result<UserInfo, anyhow::Error> {
        println!("Starting authentication process...");

        // Create a channel to receive the auth response
        let (token_tx, token_rx) = oneshot::channel::<AuthResponse>();

        // Set up the local HTTP server
        let app_state = Arc::new(AppState {
            token_tx: Mutex::new(Some(token_tx)),
        });

        // Create the router with our callback handler for both GET and POST
        let app = Router::new()
            .route("/auth", get(auth_callback_handler).post(auth_post_handler))
            .with_state(app_state.clone());

        // Start the local server
        let addr = SocketAddr::from(([127, 0, 0, 1], LOCAL_PORT));
        println!("Starting local server on http://localhost:{}", LOCAL_PORT);

        // Spawn the server in a separate task
        let server = tokio::spawn(async move {
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });

        // Wait a moment for the server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Skip temporary token request and go directly to Google OAuth
        // The server doesn't have the /auth/temp-token and /auth/cli-login endpoints yet
        println!("Preparing direct OAuth authentication...");

        // Create the callback URL for the local server
        let redirect_uri = format!("http://localhost:{}/auth", LOCAL_PORT);

        // Create the auth URL for session auth. If not present server will require google auth
        let auth_url = format!(
            "{}/auth/cli-token?redirect_uri={}",
            self.server_url,
            urlencoding::encode(&redirect_uri)
        );

        println!("Using OAuth URL: {}", auth_url);

        // Open the browser to start the auth flow
        println!("Opening browser to authenticate...");
        if let Err(e) = webbrowser::open(&auth_url) {
            println!("Failed to open browser automatically: {}", e);
            println!("Please open this URL manually: {}", auth_url);
        }

        println!("Waiting for authentication to complete...");

        let response = tokio::time::timeout(
            tokio::time::Duration::from_secs(120), // 2 minute timeout
            token_rx,
        )
        .await;
        server.abort();
        // Wait for the auth response from the callback
        let auth_response = match response {
            Ok(result) => match result {
                Ok(response) => response,
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Authentication channel closed unexpectedly"
                    ))
                }
            },
            Err(_) => return Err(anyhow::anyhow!("Authentication timed out after 2 minutes")),
        };

        // Save the token for future use
        save_token(&auth_response.token)?;

        println!("Authentication successful!");
        println!("Logged in as: {}", auth_response.user.email);

        Ok(auth_response.user)
    }
}

/// Handler for the auth callback endpoint
async fn auth_callback_handler(
    Query(params): Query<HashMap<String, String>>,
    state: axum::extract::State<Arc<AppState>>,
) -> Html<String> {
    let mut success = false;
    let mut message = "Authentication failed: No authorization data received".to_string();

    // The server passes data via URL fragment (#data=base64_encoded_json)
    // We can't access fragments directly from the server, but we'll
    // include JavaScript in our response to extract and process the fragment

    // Extract auth data directly if in params for backward compatibility
    if let Some(data) = params.get("data") {
        if process_auth_data(data, &state) {
            success = true;
            message = "Authentication successful! You can close this window.".to_string();
        }
    } else {
        // We'll use JavaScript to access the fragment and send it back to the server
        // The actual processing will happen in the JavaScript code
        println!("No data parameter in query string, will check URL fragment with JavaScript");
    }

    // Create a simple HTML response
    let html = format!(
        r#"<!DOCTYPE html>
        <html>
        <head>
            <title>Termineer Authentication</title>
            <style>
                body {{ font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto; padding: 20px; }}
                h1 {{ color: #333; }}
                .success {{ color: #28a745; }}
                .error {{ color: #dc3545; }}
            </style>
        </head>
        <body>
            <h1>Termineer Authentication</h1>
            <div id="message" class="{}">
                <p>{}</p>
                <p>You can close this window now.</p>
            </div>
            <script>
                // Check for data in URL fragment
                function processFragment() {{
                    const hash = window.location.hash;
                    if (hash && hash.startsWith('#data=')) {{
                        // Extract the base64 data
                        const base64Data = hash.substring(6);
                        
                        // Send to our endpoint as a POST request
                        fetch('/auth', {{
                            method: 'POST',
                            headers: {{
                                'Content-Type': 'application/json'
                            }},
                            body: JSON.stringify({{ data: base64Data }})
                        }})
                        .then(response => response.json())
                        .then(result => {{
                            if (result.success) {{
                                document.getElementById('message').className = 'success';
                                document.getElementById('message').innerHTML = 
                                    '<p>Authentication successful! You can close this window.</p>';
                            }}
                        }})
                        .catch(error => console.error('Error:', error));
                    }}
                }}
                
                // Process fragment when page loads
                processFragment();
                
                // Auto-close window after 3 seconds
                setTimeout(() => window.close(), 4000);
            </script>
        </body>
        </html>
        "#,
        if success { "success" } else { "error" },
        message
    );

    Html(html)
}

/// Process base64-encoded authentication data
/// Handler for POST requests to /auth endpoint, used to process auth data from URL fragment
async fn auth_post_handler(
    state: axum::extract::State<Arc<AppState>>,
    Json(request): Json<AuthDataRequest>,
) -> Response {
    println!("Received POST request with auth data");

    let success = process_auth_data(&request.data, &state);

    // Prepare response
    let response = AuthDataResponse {
        success,
        message: if success {
            "Authentication successful".to_string()
        } else {
            "Authentication failed".to_string()
        },
    };

    // Return as JSON
    (StatusCode::OK, Json(response)).into_response()
}

/// Process base64-encoded authentication data
fn process_auth_data(base64_data: &str, state: &Arc<AppState>) -> bool {
    println!("Processing auth data from base64");

    // Decode the base64 data
    match base64::decode(base64_data) {
        Ok(decoded) => {
            // Convert to string
            match String::from_utf8(decoded) {
                Ok(json_str) => {
                    println!("Successfully decoded auth data JSON");

                    // Parse the JSON
                    match serde_json::from_str::<AuthResponse>(&json_str) {
                        Ok(auth_response) => {
                            println!(
                                "Successfully parsed auth response for user: {}",
                                auth_response.user.email
                            );

                            // Send the auth response through the channel
                            if let Some(tx) = state.token_tx.lock().unwrap().take() {
                                if tx.send(auth_response).is_ok() {
                                    return true;
                                } else {
                                    println!("Failed to send auth response through channel");
                                }
                            } else {
                                println!("Token sender was already taken");
                            }
                        }
                        Err(e) => println!("Failed to parse auth response JSON: {}", e),
                    }
                }
                Err(e) => println!("Failed to convert decoded data to string: {}", e),
            }
        }
        Err(e) => println!("Failed to decode base64 data: {}", e),
    }

    false
}

/// Get the path to the auth token storage file
fn get_token_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("termineer")
        .join("auth.token")
}

/// Save the authentication token to disk
fn save_token(token: &str) -> Result<(), anyhow::Error> {
    let token_path = get_token_path();

    // Create parent directory if it doesn't exist
    if let Some(parent) = token_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Check if there's an existing token
    let existing_token = std::fs::read_to_string(&token_path).ok();
    if let Some(old_token) = existing_token {
        if old_token == token {
            println!("Token unchanged - keeping existing token");
            return Ok(());
        } else {
            println!(
                "Replacing old token (length: {}) with new token (length: {})",
                old_token.len(),
                token.len()
            );
        }
    } else {
        println!("No existing token found - saving new token");
    }

    // Write token to file with proper permissions (mode 0600 - owner read/write only)
    std::fs::write(&token_path, token)?;

    // Verify the token was written correctly
    match std::fs::read_to_string(&token_path) {
        Ok(saved_token) if saved_token == token => {
            println!("Token saved successfully to: {}", token_path.display());
            Ok(())
        }
        Ok(_) => {
            println!("WARNING: Token verification failed - saved content doesn't match");
            Err(anyhow::anyhow!(
                "Token verification failed - saved content doesn't match"
            ))
        }
        Err(e) => {
            println!("ERROR: Failed to verify saved token: {}", e);
            Err(anyhow::anyhow!("Failed to verify saved token: {}", e))
        }
    }
}

/// Load the authentication token from disk
pub fn load_token() -> Option<String> {
    let token_path = get_token_path();
    println!("Looking for auth token at: {}", token_path.display());

    let result = std::fs::read_to_string(&token_path);
    match &result {
        Ok(content) => {
            let preview = if content.len() > 20 {
                format!("{}...", &content[0..20])
            } else {
                content.clone()
            };
            println!(
                "Token found, length: {}, preview: {}",
                content.len(),
                preview
            );
        }
        Err(e) => {
            println!("Error loading token: {}", e);

            // Check if the parent directory exists for debugging
            if let Some(parent) = token_path.parent() {
                if parent.exists() {
                    println!("Parent directory exists: {}", parent.display());
                } else {
                    println!("Parent directory does not exist: {}", parent.display());
                }
            }

            // Check if there's a token in the Application Support directory directly
            if let Some(app_support) = dirs::data_local_dir() {
                let alt_path = app_support.join("termineer").join("auth.token");
                println!("Checking alternative path: {}", alt_path.display());
                if alt_path.exists() {
                    println!("Token found at alternative path!");
                }
            }
        }
    }

    result.ok()
}

/// Check if a user subscription is expired
pub fn is_subscription_expired(user_info: &UserInfo) -> bool {
    if let Some(expires_at) = user_info.expires_at {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        expires_at < now
    } else {
        // If no expiration is provided, consider it not expired
        false
    }
}

// No longer needed imports have been removed
