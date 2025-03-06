use axum::{
    routing::{get, post},
    Router,
    extract::Json,
    response::{Redirect, IntoResponse},
    http::{StatusCode, HeaderMap, header},
    body::Body,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use serde_json::json;

// Verbose logging for debugging
macro_rules! server_log {
    ($($arg:tt)*) => {
        println!("MOCK SERVER: {}", format!($($arg)*));
    }
}

// Mock authentication server for testing

// Simple structure to mock user information
#[derive(Serialize)]
struct UserInfo {
    email: String,
    display_name: Option<String>,
    subscription_type: Option<String>,
    subscription_status: Option<String>,
    expires_at: Option<i64>,
    features: Vec<String>,
}

// Simple structure for token response
#[derive(Serialize)]
struct TokenResponse {
    token: String,
    user: UserInfo,
    redirect_url: String,
}

// Handle Google login redirect
async fn google_login() -> impl IntoResponse {
    server_log!("Received login request, redirecting to callback with mock token");
    
    // Get frontend URL from environment variable or use default
    let frontend_url = std::env::var("FRONTEND_URL")
        .unwrap_or_else(|_| "http://localhost:8732".to_string());
    
    // Construct the full callback URL with token
    let callback_url = format!("{}/callback?token=mock_test_token_12345", frontend_url);
    server_log!("Redirecting to: {}", callback_url);
    
    // Redirect to callback URL with a mock token
    Redirect::to(&callback_url)
}

// Handle token verification
#[derive(Deserialize)]
struct VerifyRequest {
    token: String,
}

// Handle token verification
async fn verify_token(Json(req): Json<VerifyRequest>) -> impl IntoResponse {
    server_log!("Verifying token: {}", req.token);
    
    // Return success for any token for testing
    (StatusCode::OK, Json(json!({
        "valid": true,
        "user": {
            "id": "1234-5678-9012",
            "email": "test@example.com",
            "name": "Test User",
            "has_subscription": true
        }
    })))
}

// Handle user info requests
async fn user_info(headers: HeaderMap) -> impl IntoResponse {
    // Log all headers for debugging
    server_log!("User info requested with headers:");
    for (name, value) in headers.iter() {
        server_log!("  {}: {}", name, value.to_str().unwrap_or("Invalid UTF-8"));
    }
    
    // Check environment
    let env = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());
    server_log!("Current environment: {}", env);
    
    // Check for Authorization header
    if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
        let auth_value = auth_header.to_str().unwrap_or("invalid");
        server_log!("Authorization header: {}", auth_value);
        
        // Extract the token (should be in the format "Bearer <token>")
        if auth_value.starts_with("Bearer ") {
            let token = &auth_value[7..]; // Skip "Bearer "
            server_log!("Token extracted: {}", token);
            
            // For testing, accept any token (should validate in production)
            // In production, we would verify the token's signature
            let current_time = chrono::Utc::now();
            let expiration = current_time + chrono::Duration::days(30);
            
            // Return mock user data
            server_log!("Returning successful user info response");
            
            // Construct user response
            let response = json!({
                "email": "test@example.com",
                "display_name": "Test User",
                "subscription_type": "premium",
                "subscription_status": "active",
                "expires_at": expiration.timestamp(),
                "features": ["all"]
            });
            
            server_log!("Response payload: {}", response.to_string());
            return (StatusCode::OK, Json(response));
        }
    }
    
    // If no proper authorization header, return error
    server_log!("Error: No valid authorization header found");
    let error_response = json!({
        "error": "Unauthorized",
        "message": "No valid authorization token provided"
    });
    server_log!("Error response: {}", error_response.to_string());
    
    (
        StatusCode::UNAUTHORIZED,
        Json(error_response)
    )
}

// Run the mock server
pub async fn run_simple_server() -> Result<(), Box<dyn std::error::Error>> {
    // Define health check handler
    async fn health_check() -> impl IntoResponse {
        server_log!("Health check requested");
        "OK - Server is healthy"
    }

    // Build our application with routes
    let app = Router::new()
        .route("/auth/google/login", get(google_login))
        .route("/auth/verify", post(verify_token))
        .route("/auth/user", get(user_info))
        .route("/health", get(health_check)); // Explicit health check endpoint

    // Get port from environment or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);
    
    // Bind to all interfaces (0.0.0.0) to allow external connections
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    server_log!("Starting OAuth server on {} (port {})", addr, port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}