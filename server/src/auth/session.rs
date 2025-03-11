//! Session management
//!
//! Handles user session creation, validation, and retrieval.

use crate::AppState;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

/// Cookie name for session token
const SESSION_COOKIE: &str = "termineer_session";
/// Secret key for JWT encoding/decoding (should be configured from environment in production)
const JWT_SECRET: &str = "termineer_jwt_secret_change_this_in_production";
/// Session duration in seconds (24 hours)
const SESSION_DURATION: u64 = 86400;

/// Session claims for JWT
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionClaims {
    /// JWT subject (user ID)
    pub sub: String,
    /// User email
    pub email: String,
    /// Name of the user (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// User profile picture URL (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// JWT expiration time
    pub exp: u64,
    /// JWT issued at time
    pub iat: u64,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID
    pub id: String,
    /// User email
    pub email: String,
    /// User name (optional)
    pub name: Option<String>,
    /// Profile picture URL (optional)
    pub picture: Option<String>,
    /// Subscription type (optional)
    pub subscription: Option<String>,
}

/// Create a new session for a user
pub fn create_session(
    cookies: &Cookies,
    user_id: &str,
    email: &str,
    name: Option<&str>,
    picture: Option<&str>,
) -> Result<(), String> {
    // Get current time
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    // Create session claims
    let claims = SessionClaims {
        sub: user_id.to_string(),
        email: email.to_string(),
        name: name.map(|s| s.to_string()),
        picture: picture.map(|s| s.to_string()),
        exp: now + SESSION_DURATION,
        iat: now,
    };

    // Encode JWT
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET.as_bytes()),
    )
    .map_err(|e| e.to_string())?;

    // Create cookie
    let cookie = Cookie::build(SESSION_COOKIE, token)
        .path("/")
        .max_age(time::Duration::new(SESSION_DURATION, 0))
        .http_only(true)
        .secure(false) // Set to true in production with HTTPS
        .finish();

    // Add cookie to response
    cookies.add(cookie);

    Ok(())
}

/// Get user information from session
pub fn get_user_from_session(cookies: &Cookies) -> Option<UserInfo> {
    // Get session cookie
    let cookie = cookies.get(SESSION_COOKIE)?;

    // Decode JWT
    let token_data = decode::<SessionClaims>(
        cookie.value(),
        &DecodingKey::from_secret(JWT_SECRET.as_bytes()),
        &Validation::default(),
    )
    .ok()?;

    // Create user info
    Some(UserInfo {
        id: token_data.claims.sub,
        email: token_data.claims.email,
        name: token_data.claims.name,
        picture: token_data.claims.picture,
        subscription: None, // We'll need to fetch this from the database in a real implementation
    })
}

/// Clear session cookie
pub fn clear_session(cookies: &Cookies) {
    let cookie = Cookie::build(SESSION_COOKIE, "")
        .path("/")
        .max_age(time::Duration::new(0, 0))
        .http_only(true)
        .secure(false) // Set to true in production with HTTPS
        .finish();

    cookies.add(cookie);
}

/// Session data for use in templates
#[derive(Debug, Clone, Default)]
pub struct SessionData {
    /// User information (None if not logged in)
    pub user: Option<UserInfo>,
}

/// Middleware to extract session data
pub async fn session_data_middleware(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
    request: Request,
    next: Next,
) -> Response {
    let user = get_user_from_session(&cookies);
    let session_data = SessionData { user };

    // Store session data in request extensions
    let mut request = request;
    request.extensions_mut().insert(session_data);

    next.run(request).await
}