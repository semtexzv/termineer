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
mod templates;

// Re-export AppState from api module to make it available at crate root
pub use api::AppState;

use auth::oauth;
use axum::{
    routing::{get, post},
    extract::{Form, Extension},
    response::{IntoResponse, Html},
    Router,
};
use askama::Template; // Add this import
use config::Config;
use middleware::attach_user;
use sqlx::postgres::PgConnectOptions;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use log::LevelFilter;
use sqlx::ConnectOptions;
use templates::{IndexTemplate, AuthButtonTemplate, CheckoutTemplate, User};
use tower_http::services::ServeDir;
use tower_sessions::{cookie::SameSite, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

// Handle root route - serve index page
async fn index_handler() -> impl IntoResponse {
    IndexTemplate
}

// Auth status handler for HTMX
async fn auth_status_handler(Extension(user): Extension<Option<User>>) -> impl IntoResponse {
    AuthButtonTemplate { user }
}

// Checkout form handler - We need to explicitly use askama's rendering
async fn checkout_handler(
    Extension(user): Extension<Option<User>>,
    Form(params): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let plan = params.get("plan").cloned().unwrap_or_else(|| "free".to_string());
    
    // Map plan to display name
    let plan_name = match plan.as_str() {
        "free" => "free".to_string(),
        "plus" => "plus".to_string(),
        "pro" => "pro".to_string(),
        _ => "unknown".to_string(),
    };
    
    // Generate checkout URL
    let checkout_url = format!("/payment/stripe-checkout");
    
    // Get user email if logged in
    let email = user.as_ref().map(|u| u.email.clone()).unwrap_or_default();
    
    // Create the template and render it
    let template = CheckoutTemplate {
        plan_name,
        checkout_url,
        email,
    };
    
    match template.render() {
        Ok(html) => Html(html),
        Err(_) => Html("Error rendering template".to_string()),
    }
}

// Cancel checkout handler
async fn cancel_checkout() -> impl IntoResponse {
    Html("".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Starting server...");
    // Load configuration from environment variables
    let config = Config::from_env()?;

    // Initialize logging based on environment
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info,sqlx=warn");
    }
    tracing_subscriber::fmt::init();

    // Initialize database connection
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    eprintln!("Connecting to database...{db_url}");

    let opts = PgConnectOptions::from_str(&config.database_url)?.log_statements(LevelFilter::Trace);

    let pool = sqlx::PgPool::connect_with(opts).await?;

    // Ensure database schema is up to date by running migrations
    // In Docker environment, migrations are stored in /etc/autoswe/migrations
    // If that directory doesn't exist, use local migrations
    let migrations_path = if std::path::Path::new("/etc/autoswe/migrations").exists() {
        "/etc/autoswe/migrations"
    } else {
        "./migrations"
    };

    sqlx::migrate::Migrator::new(std::path::Path::new(migrations_path))
        .await
        .map_err(|e| {
            eprintln!("Error loading migrations: {}", e);
            e
        })?
        .run(&pool)
        .await
        .map_err(|e| {
            eprintln!("Error running migrations: {}", e);
            e
        })?;

    // Create a shared application state
    let state = Arc::new(AppState {
        db_pool: pool.clone(),
        config: config.clone(),
    });

    // Session store for OAuth authentication
    let session_store = PostgresStore::new(pool.clone());
    session_store.migrate().await?;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(true) // Set to true in production with HTTPS
        .with_same_site(SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(24)));

    // Get static files directory from environment or use default
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());
    
    // Setup routes
    let app = Router::new()
        // Frontend routes
        .route("/", get(index_handler))
        .route("/auth/status", get(auth_status_handler))
        .route("/payment/checkout", post(checkout_handler))
        .route("/payment/stripe-checkout", post(payment::stripe::create_checkout))
        .route("/payment/success-page", get(payment::stripe::success_page)) // New non-authenticated success page
        .route("/cancel-checkout", get(cancel_checkout))
        // Serve static files
        .nest_service("/static", ServeDir::new(static_dir))
        // Authentication routes
        .route("/auth/google/login", get(oauth::google_login))
        .route("/auth/google/callback", get(oauth::google_callback))
        .route("/auth/user", get(api::auth::get_current_user))
        // Mock license routes (for development and testing)
        .route("/license/verify", post(api::mock_license::verify_license))
        // Payment routes - Keep the authenticated version too
        .route("/payment/success", get(api::payment::get_subscription))
        .route("/payment/webhook", post(payment::stripe::handle_webhook))
        // Health check
        .route("/health", get(|| async { "OK - Server is healthy" }))
        // Apply middleware for user attachment
        .layer(axum::middleware::from_fn(attach_user))
        // Apply application state
        .with_state(state)
        // Apply session layer
        .layer(session_layer);

    // Start the server
    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(3000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}