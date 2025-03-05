use axum::{
    routing::get,
    Router,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{CorsLayer, Any};
use tower_sessions::{SessionManagerLayer, Session, Expiry};
use time::Duration;
use tower_sessions_memory_store;

mod config;
mod auth;
mod payment;
mod db;
mod api;
mod errors;
mod middleware;

use config::Config;
use errors::ServerError;

/// Application state that is shared across all routes
#[derive(Clone)]
pub struct AppState {
    db_pool: sqlx::PgPool,
    config: Arc<Config>,
}

// Simplified handler function for demo
async fn hello_world() -> impl IntoResponse {
    (StatusCode::OK, Json(serde_json::json!({ "message": "Hello, world!" })))
}

// Healthcheck endpoint
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}

// API info endpoint
async fn api_info() -> impl IntoResponse {
    let info = serde_json::json!({
        "name": "AutoSWE Server",
        "version": "0.1.0",
        "description": "Authentication and payment processing for AutoSWE",
        "endpoints": {
            "/health": "Health check endpoint",
            "/api/info": "API information",
        }
    });
    
    (StatusCode::OK, Json(info))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load environment variables
    dotenv::dotenv().ok();
    
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    
    println!("Starting AutoSWE server...");
    
    // Load configuration
    let config = match Config::from_env() {
        Ok(config) => {
            println!("Configuration loaded successfully");
            Arc::new(config)
        },
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            return Err(e.into());
        }
    };
    
    // Set up database connection pool
    let pool = match sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url).await {
            Ok(pool) => {
                println!("Database connection established");
                pool
            },
            Err(e) => {
                eprintln!("Failed to connect to database: {}", e);
                return Err(e.into());
            }
    };
    
    // Initialize the application state
    let app_state = AppState {
        db_pool: pool.clone(),
        config: config.clone(),
    };
    
    // Configure CORS
    let cors = CorsLayer::new()
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(Any)
        .allow_origin(Any); // In production, limit this to specific origins
    
    // Configure session management - use in-memory store for testing
    let session_store = tower_sessions_memory_store::MemoryStore::default();
    
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true in production
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::seconds(60 * 60 * 24))); // 24 hours
    
    // Build our application with a route
    let app = Router::new()
        // Add simplified routes for demo
        .route("/", get(hello_world))
        .route("/health", get(health_check))
        .route("/api/info", get(api_info))
        
        // OAuth authentication routes
        .route("/auth/google/login", get(auth::oauth::google_login))
        .route("/auth/google/callback", get(auth::oauth::google_callback))
        
        // License verification routes
        .route("/license/verify", axum::routing::post(verify_license_handler))
        
        // Add middleware
        .layer(cors)
        .layer(session_layer)
        .with_state(app_state);
    
    // Run the server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    println!("Server listening on {}", addr);
    
    // Create Hyper server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    
    Ok(())
}

/// Request for license verification
#[derive(serde::Deserialize)]
struct VerifyLicenseRequest {
    license_key: String,
    client_id: String,
}

/// Response for license verification
#[derive(serde::Serialize)]
struct VerifyLicenseResponse {
    valid: bool,
    user_email: Option<String>,
    subscription_type: Option<String>,
    expires_at: Option<i64>,
    features: Vec<String>,
    message: Option<String>,
}

/// Valid test license keys with associated data
const TEST_LICENSES: [(&str, &str, &str, bool); 3] = [
    // (key, email, subscription_type, is_valid)
    ("TEST-DEV-LICENSE-KEY", "developer@example.com", "developer", true),
    ("TEST-PRO-LICENSE-KEY", "pro@example.com", "professional", true),
    ("TEST-EXPIRED-LICENSE", "expired@example.com", "basic", false),
];

/// License verification handler
async fn verify_license_handler(Json(req): Json<VerifyLicenseRequest>) -> impl IntoResponse {
    println!("License verification request for key: {}", req.license_key);
    
    // Default response (invalid)
    let mut response = VerifyLicenseResponse {
        valid: false,
        user_email: None,
        subscription_type: None,
        expires_at: None,
        features: Vec::new(),
        message: Some("Invalid license key".to_string()),
    };
    
    // Current time
    let now = chrono::Utc::now();
    
    // Check against test license keys
    for (key, email, subscription, is_valid) in TEST_LICENSES.iter() {
        if &req.license_key == key {
            // Calculate expiration date based on validity
            let expires_at = if *is_valid {
                // Valid keys expire 1 year from now
                now + chrono::Duration::days(365)
            } else {
                // Expired keys expired 30 days ago
                now - chrono::Duration::days(30)
            };
            
            response = VerifyLicenseResponse {
                valid: *is_valid,
                user_email: Some(email.to_string()),
                subscription_type: Some(subscription.to_string()),
                expires_at: Some(expires_at.timestamp()),
                features: get_features_for_subscription(subscription),
                message: if *is_valid {
                    None
                } else {
                    Some("License has expired".to_string())
                },
            };
            break;
        }
    }
    
    // If using DEV-MODE flag, consider any key starting with DEV- valid for testing
    if !response.valid && req.license_key.starts_with("DEV-") {
        response = VerifyLicenseResponse {
            valid: true,
            user_email: Some("developer@localhost".to_string()),
            subscription_type: Some("development".to_string()),
            expires_at: Some((now + chrono::Duration::days(30)).timestamp()),
            features: vec!["all".to_string()],
            message: None,
        };
    }
    
    println!("License verification result: {}", response.valid);
    (StatusCode::OK, Json(response))
}

/// Get features available for a subscription type
fn get_features_for_subscription(subscription_type: &str) -> Vec<String> {
    match subscription_type {
        "developer" => vec![
            "basic".to_string(),
            "advanced".to_string(),
            "developer".to_string(),
        ],
        "professional" => vec![
            "basic".to_string(),
            "advanced".to_string(),
            "professional".to_string(),
        ],
        _ => vec!["basic".to_string()],
    }
}