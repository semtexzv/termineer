use axum::{
    extract::{Query, State},
    response::{Redirect, IntoResponse},
    http::StatusCode,
    Json,
};
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use oauth2::basic::BasicClient;
use oauth2::reqwest::async_http_client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower_sessions::Session;
use futures::TryFutureExt;
use log::{info, error};
use uuid::Uuid;

use crate::errors::ServerError;
use crate::config::Config;
use crate::db::models::User;
use crate::db::operations::UserOps;
use crate::auth::jwt::create_token;
use crate::AppState;

// Session key for storing PKCE code verifier
const PKCE_VERIFIER_KEY: &str = "pkce_verifier";

/// Structure for user info returned from Google
#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: String,
    name: Option<String>,
    picture: Option<String>,
    verified_email: Option<bool>,
}

/// Response structure for successful authentication
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    token: String,
    user: UserResponse,
}

/// User information for response
#[derive(Debug, Serialize)]
pub struct UserResponse {
    id: Uuid,
    email: String,
    name: Option<String>,
    is_active: bool,
    has_subscription: bool,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        UserResponse {
            id: user.id,
            email: user.email,
            name: user.name,
            is_active: user.is_active,
            has_subscription: user.has_subscription,
        }
    }
}

/// Initiates the OAuth 2.0 flow with Google
pub async fn google_login(
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ServerError> {
    // Create OAuth2 client for Google
    let client = create_google_oauth_client(&state.config);
    
    // Generate PKCE challenge
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    
    // Generate CSRF token
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();
    
    // Store PKCE verifier in session for later verification
    session.insert(PKCE_VERIFIER_KEY, pkce_verifier.secret())
        .await
        .map_err(|e| {
            error!("Session error: {}", e);
            ServerError::Internal("Failed to create session".to_string())
        })?;
    
    session.insert("csrf_token", csrf_token.secret())
        .await
        .map_err(|e| {
            error!("Session error: {}", e);
            ServerError::Internal("Failed to create session".to_string())
        })?;
    
    // Redirect to Google's authorization page
    Ok(Redirect::to(auth_url.as_str()))
}

/// Handles the callback from Google after user authentication
pub async fn google_callback(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
    session: Session,
) -> Result<impl IntoResponse, ServerError> {
    // Extract code and state from query parameters
    let code = params.get("code")
        .ok_or_else(|| ServerError::Auth("No authorization code found".to_string()))?;
    let state_param = params.get("state")
        .ok_or_else(|| ServerError::Auth("No state found".to_string()))?;
    
    // Verify CSRF token
    let csrf_token = session.get::<String>("csrf_token")
        .await
        .map_err(|e| ServerError::Internal(format!("Session error: {}", e)))?
        .ok_or_else(|| ServerError::Auth("CSRF token not found in session".to_string()))?;
    
    if state_param != &csrf_token {
        return Err(ServerError::Auth("CSRF token mismatch".to_string()));
    }
    
    // Get PKCE verifier from session
    let verifier_secret = session.get::<String>(PKCE_VERIFIER_KEY)
        .await
        .map_err(|e| ServerError::Internal(format!("Session error: {}", e)))?
        .ok_or_else(|| ServerError::Auth("PKCE verifier not found in session".to_string()))?;
    
    let pkce_verifier = PkceCodeVerifier::new(verifier_secret);
    
    // Create OAuth client
    let client = create_google_oauth_client(&state.config);
    
    // Exchange the authorization code for an access token
    let token_result = client
        .exchange_code(AuthorizationCode::new(code.clone()))
        .set_pkce_verifier(pkce_verifier)
        .request_async(async_http_client)
        .await
        .map_err(|e| {
            error!("Token exchange error: {}", e);
            ServerError::Auth("Failed to exchange authorization code for token".to_string())
        })?;
    
    // Use the access token to get user info from Google
    let user_info = fetch_google_user_info(token_result.access_token().secret()).await?;
    
    // Validate that the email is verified
    if let Some(verified) = user_info.verified_email {
        if !verified {
            return Err(ServerError::Auth("Email not verified with Google".to_string()));
        }
    }
    
    // Create or update user in database
    let user = UserOps::find_or_create_from_oauth(
        &state.db_pool, 
        &user_info.email, 
        user_info.name, 
        "google".to_string()
    ).await?;
    
    // Create JWT token
    let token = create_token(&user, &state.config.jwt_secret, state.config.jwt_expiry)?;
    
    // Clear session data
    session.remove::<String>(PKCE_VERIFIER_KEY).await.ok();
    session.remove::<String>("csrf_token").await.ok();
    
    // Clone token before it gets moved
    let token_clone = token.clone();
    
    // Always return JSON response with appropriate redirection URL
    let response = AuthResponse {
        token,
        user: user.into(),
    };
    
    // Include the redirect URL in the response if in development mode
    let mut json_response = serde_json::json!({
        "token": token_clone,
        "user": response.user,
    });
    
    if state.config.is_development() {
        json_response["redirect_url"] = serde_json::Value::String(
            format!("{}/auth/callback?token={}", state.config.frontend_url, token_clone)
        );
    }
    
    Ok((StatusCode::OK, Json(json_response)))
}

/// Fetches user information from Google using the access token
async fn fetch_google_user_info(access_token: &str) -> Result<GoogleUserInfo, ServerError> {
    let client = reqwest::Client::new();
    let user_info_uri = "https://www.googleapis.com/oauth2/v2/userinfo";
    
    let res = client
        .get(user_info_uri)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .map_err(|e| {
            error!("Failed to fetch user info: {}", e);
            ServerError::External("Failed to fetch user info from Google".to_string())
        })?;
    
    if !res.status().is_success() {
        let status = res.status();
        let error_text = res.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Google API error: Status: {}, Body: {}", status, error_text);
        return Err(ServerError::External(format!("Google API error: {}", status)));
    }
    
    let user_info = res.json::<GoogleUserInfo>().await.map_err(|e| {
        error!("Failed to parse user info: {}", e);
        ServerError::Internal("Failed to parse user info response".to_string())
    })?;
    
    Ok(user_info)
}

/// Creates an OAuth 2.0 client for Google authentication
fn create_google_oauth_client(config: &Config) -> BasicClient {
    BasicClient::new(
        ClientId::new(config.google_client_id.clone()),
        Some(ClientSecret::new(config.google_client_secret.clone())),
        AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).unwrap(),
        Some(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).unwrap())
    )
    .set_redirect_uri(RedirectUrl::new(config.oauth_redirect_url.clone()).unwrap())
}