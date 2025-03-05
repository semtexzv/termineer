//! AutoSWE Server
//!
//! This is the main entry point for the AutoSWE server.
//! It provides authentication, payment processing, and license management.

mod api;
mod auth;
mod config;
mod db;
mod errors;
mod middleware;
mod payment;

use auth::oauth;
use axum::{
    routing::{get, post},
    Extension, Router,
};
use config::Config;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_sessions::{cookie::SameSite, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment variables
    let config = Config::from_env()?;
    
    // Initialize logging based on environment
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,sqlx=warn");
    }
    tracing_subscriber::fmt::init();
    
    // Initialize database connection
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/autoswe.db".to_string());
    
    let pool = sqlx::SqlitePool::connect(&db_url).await?;
    
    // Create the migrations directory if it doesn't exist
    std::fs::create_dir_all("./migrations").ok();
    
    // Ensure database schema is up to date
    // For SQLite, we use the specific migration file
    sqlx::query_file!("./migrations/20240305_sqlite_schema.sql")
        .execute(&pool)
        .await
        .ok(); // Ignore errors if table already exists
    
    // Create a shared application state
    let state = Arc::new(api::AppState {
        db_pool: pool.clone(),
        config: config.clone(),
    });
    
    // Session store for OAuth authentication
    let session_store = SqliteStore::new(pool);
    session_store.migrate().await?;
    
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // Set to true in production with HTTPS
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(24)));
    
    // Setup routes
    let app = Router::new()
        // Authentication routes
        .route("/auth/google/login", get(oauth::google_login_handler))
        .route("/auth/google/callback", get(oauth::google_callback_handler))
        .route("/auth/user", get(api::auth::get_current_user))
        
        // Mock license routes (for development and testing)
        .route("/license/verify", post(api::mock_license::verify_license))
        
        // Payment routes
        .route("/payment/checkout", post(api::payment::create_checkout))
        .route("/payment/success", get(api::payment::payment_success))
        .route("/payment/webhook", post(api::payment::stripe_webhook))
        
        // Health check
        .route("/health", get(|| async { "OK - Server is healthy" }))
        
        // Apply middleware
        .layer(Extension(state))
        .layer(session_layer);
    
    // Start the server
    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3000);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting server on {}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}