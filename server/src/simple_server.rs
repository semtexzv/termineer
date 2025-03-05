use axum::{
    routing::{get, post},
    Router,
    extract::Json,
    response::{Redirect, IntoResponse},
    http::StatusCode,
};
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use serde_json::json;

// Mock authentication server for testing

// Simple structure to mock user information
#[derive(Serialize)]
struct UserInfo {
    email: String,
    name: String,
    has_subscription: bool,
    subscription_type: String,
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
    println!("Received login request, redirecting to callback with mock token");
    // Redirect to callback URL with a mock token
    Redirect::to("http://localhost:8732/callback?token=mock_test_token_12345")
}

// Handle token verification
#[derive(Deserialize)]
struct VerifyRequest {
    token: String,
}

// Handle token verification
async fn verify_token(Json(req): Json<VerifyRequest>) -> impl IntoResponse {
    println!("Verifying token: {}", req.token);
    
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

// Run the mock server
pub async fn run_simple_server() -> Result<(), Box<dyn std::error::Error>> {
    // Build our application with routes
    let app = Router::new()
        .route("/auth/google/login", get(google_login))
        .route("/auth/verify", post(verify_token))
        .route("/health", get(|| async { "OK" }));

    // Run it with tokio (use port 3001 to avoid conflicts)
    let addr = SocketAddr::from(([127, 0, 0, 1], 3001));
    println!("Starting mock OAuth server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}