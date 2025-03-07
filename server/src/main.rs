//! Termineer Server
//!
//! This is the main entry point for the Termineer server.
//! Provides HTTP endpoints for the Termineer web interface.

mod api;
mod config;
mod db;
mod errors;
mod templates;

// Re-export AppState from api module to make it available at crate root
pub use api::AppState;

use axum::{
    routing::get,
    response::IntoResponse,
    Router,
};
use config::Config;
use std::net::SocketAddr;
use std::sync::Arc;
use templates::{IndexTemplate, ManualTemplate};
use tower_http::services::ServeDir;

// Handle root route - serve index page
async fn index_handler() -> impl IntoResponse {
    IndexTemplate
}

// Handle manual route - serve manual page
async fn manual_handler() -> impl IntoResponse {
    ManualTemplate
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration from environment variables
    let config = Config::from_env()?;

    // Initialize logging based on environment
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,sqlx=warn");
    }
    tracing_subscriber::fmt::init();

    // Connect to the database
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .map_err(|e| {
            tracing::error!("Failed to connect to database: {}", e);
            e
        })?;

    // Create a shared application state
    let state = Arc::new(AppState {
        db_pool: pool,
        config: config.clone(),
    });

    // Get static files directory from environment or use default
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    
    // Create the router
    let app = Router::new()
        // Frontend routes
        .route("/", get(index_handler))
        .route("/manual", get(manual_handler))
        // Serve static files
        .nest_service("/static", ServeDir::new(static_dir))
        // Health check
        .route("/health", get(|| async { "OK" }))
        // Apply state to the router
        .with_state(state);
    
    // Get the port from environment or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8080);

    // Create the server address
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}