use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use log::{info, error};

use crate::auth::jwt::{AuthenticatedUser, create_token, verify_token as jwt_verify_token};
use crate::AppState;
use crate::config::Config;
use crate::db::operations::UserOps;
use crate::errors::ServerError;

/// Response for user information
#[derive(Debug, Serialize)]
pub struct UserResponse {
    id: Uuid,
    email: String,
    name: Option<String>,
    has_subscription: bool,
}

/// Get information about the current authenticated user
pub async fn get_current_user(
    auth_user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // Look up the latest user information from the database
    let db_user = UserOps::find_by_id(&state.db_pool, auth_user.user_id).await?
        .ok_or_else(|| ServerError::NotFound("User not found".to_string()))?;
    
    let response = UserResponse {
        id: db_user.id,
        email: db_user.email,
        name: db_user.name,
        has_subscription: db_user.has_subscription,
    };
    
    Ok((StatusCode::OK, Json(response)))
}

/// Request for token verification
#[derive(Debug, Deserialize)]
pub struct VerifyTokenRequest {
    token: String,
}

/// Response for token verification
#[derive(Debug, Serialize)]
pub struct VerifyTokenResponse {
    valid: bool,
    user: Option<UserResponse>,
}

/// Verify a JWT token
pub async fn verify_token(
    Json(req): Json<VerifyTokenRequest>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // Try to verify the token
    match jwt_verify_token(&req.token, &state.config.jwt_secret) {
        Ok(claims) => {
            // Token is valid, look up the user
            let user_id = Uuid::parse_str(&claims.sub)
                .map_err(|_| ServerError::Auth("Invalid user ID in token".to_string()))?;
            
            let user = UserOps::find_by_id(&state.db_pool, user_id).await?;
            
            match user {
                Some(user) => {
                    // Return user information
                    let user_response = UserResponse {
                        id: user.id,
                        email: user.email,
                        name: user.name,
                        has_subscription: user.has_subscription,
                    };
                    
                    Ok((StatusCode::OK, Json(VerifyTokenResponse {
                        valid: true,
                        user: Some(user_response),
                    })))
                },
                None => {
                    // Token is valid but user not found
                    Ok((StatusCode::OK, Json(VerifyTokenResponse {
                        valid: false,
                        user: None,
                    })))
                }
            }
        },
        Err(_) => {
            // Token is invalid
            Ok((StatusCode::OK, Json(VerifyTokenResponse {
                valid: false,
                user: None,
            })))
        }
    }
}

/// Request for refreshing a token
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    token: String,
}

/// Response for token refresh
#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    token: String,
}

/// Refresh a JWT token
pub async fn refresh_token(
    Json(req): Json<RefreshTokenRequest>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // Verify the current token
    let claims = jwt_verify_token(&req.token, &state.config.jwt_secret)?;
    
    // Parse the user ID
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| ServerError::Auth("Invalid user ID in token".to_string()))?;
    
    // Look up the user
    let user = UserOps::find_by_id(&state.db_pool, user_id).await?
        .ok_or_else(|| ServerError::NotFound("User not found".to_string()))?;
    
    // Create a new token
    let new_token = create_token(&user, &state.config.jwt_secret, state.config.jwt_expiry)?;
    
    Ok((StatusCode::OK, Json(RefreshTokenResponse {
        token: new_token,
    })))
}