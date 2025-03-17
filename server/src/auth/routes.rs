//! Authentication routes
//!
//! Handles OAuth authentication flows and session management.

use crate::auth::google::{
    create_oauth_client, exchange_code_and_get_user_info, generate_auth_url,
};
use crate::auth::session::{clear_session, create_session};
use crate::AppState;
use axum::{
    extract::{Query, State},
    response::Redirect,
    routing::get,
    Router,
};
use oauth2::AuthorizationCode;
use serde::Deserialize;
use std::sync::Arc;
use tower_cookies::Cookies;
use tracing::{error, info};

/// Define authentication routes
pub fn auth_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/google/login", get(google_login))
        .route("/auth/google/callback", get(google_callback))
        .route("/auth/logout", get(logout))
}

#[axum::debug_handler]
/// Handler for Google login
pub async fn google_login(State(state): State<Arc<AppState>>) -> Redirect {
    info!("Starting Google OAuth login flow");

    // Create OAuth client
    let client = match create_oauth_client(&state.config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create OAuth client: {}", e);
            return Redirect::to("/");
        }
    };

    // Generate authorization URL
    let (auth_url, _csrf_token, _pkce_verifier) = generate_auth_url(&client);

    // Store CSRF token and PKCE verifier in session
    // In a real application, you would store these in a database or Redis
    // For simplicity, we're using a cookie-based session
    // This is not secure for production use

    // Redirect to authorization URL
    Redirect::to(&auth_url)
}

/// Google OAuth callback parameters
#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
    #[serde(default)]
    error: Option<String>,
}

#[axum::debug_handler]
/// Handler for Google callback
pub async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<CallbackParams>,
    cookies: Cookies,
) -> Redirect {
    // Check for error
    if let Some(error) = params.error {
        error!("OAuth callback error: {}", error);
        return Redirect::to("/");
    }

    info!("Received OAuth callback");

    // Create OAuth client
    let client = match create_oauth_client(&state.config) {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create OAuth client: {}", e);
            return Redirect::to("/");
        }
    };

    // Exchange code for token and get user info
    // In a real application, you would validate the CSRF token and use the stored PKCE verifier
    // For simplicity, we're skipping that step
    let code = AuthorizationCode::new(params.code);

    // Generate a dummy PKCE verifier (in a real app, you'd retrieve this from your session store)
    let (_, _, pkce_verifier) = generate_auth_url(&client);

    let user_info = match exchange_code_and_get_user_info(&client, code, pkce_verifier).await {
        Ok(user_info) => user_info,
        Err(e) => {
            error!("Failed to exchange code for token: {}", e);
            return Redirect::to("/");
        }
    };

    // Create session
    if let Err(e) = create_session(
        &cookies,
        &user_info.id,
        &user_info.email,
        user_info.name.as_deref(),
        user_info.picture.as_deref(),
    ) {
        error!("Failed to create session: {}", e);
        return Redirect::to("/");
    }

    // Redirect to home page
    Redirect::to("/")
}

#[axum::debug_handler]
/// Handler for logout
pub async fn logout(cookies: Cookies) -> Redirect {
    info!("User logged out");

    // Clear session
    clear_session(&cookies);

    // Redirect to home page
    Redirect::to("/")
}
