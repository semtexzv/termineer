use axum::{
    async_trait,
    extract::{FromRequestParts, Request},
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::{Response, IntoResponse},
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::models::User;
use crate::errors::ServerError;
use crate::AppState;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Email address
    pub email: String,
    /// User name (optional)
    pub name: Option<String>,
    /// Has active subscription
    pub has_subscription: bool,
    /// Issued at timestamp
    pub iat: i64,
    /// Expiration timestamp
    pub exp: i64,
}

/// Represents an authenticated user
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub has_subscription: bool,
}

/// Implement extractor for AuthenticatedUser from HTTP request
#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract the token from the Authorization header
        let auth_header = parts
            .headers
            .get("Authorization")
            .ok_or_else(|| ServerError::Auth("Missing authorization header".to_string()))?;

        let auth_str = auth_header
            .to_str()
            .map_err(|_| ServerError::Auth("Invalid authorization header".to_string()))?;

        // Check if it's a bearer token
        if !auth_str.starts_with("Bearer ") {
            return Err(ServerError::Auth("Invalid authorization scheme".to_string()));
        }

        // Extract the token
        let token = &auth_str[7..]; // Skip "Bearer "

        // Verify the token
        let config = match parts.extensions.get::<AppState>() {
            Some(config) => config,
            None => return Err(ServerError::Internal("Server configuration error".to_string())),
        };

        // Validate the token
        let claims = verify_token(token, &config.config.jwt_secret)?;

        // Convert sub to UUID
        let user_id = Uuid::parse_str(&claims.sub)
            .map_err(|_| ServerError::Auth("Invalid user ID in token".to_string()))?;

        // Return the authenticated user
        Ok(AuthenticatedUser {
            user_id,
            email: claims.email,
            name: claims.name,
            has_subscription: claims.has_subscription,
        })
    }
}

/// Creates a JWT token for a user
pub fn create_token(user: &User, secret: &str, expiry_seconds: i64) -> Result<String, ServerError> {
    let now = Utc::now();
    let iat = now.timestamp();
    let exp = (now + Duration::seconds(expiry_seconds)).timestamp();
    
    let claims = Claims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        name: user.name.clone(),
        has_subscription: user.has_subscription,
        iat,
        exp,
    };
    
    let header = Header::new(Algorithm::HS256);
    let encoding_key = EncodingKey::from_secret(secret.as_bytes());
    
    encode(&header, &claims, &encoding_key)
        .map_err(|e| ServerError::Internal(format!("Failed to create token: {}", e)))
}

/// Verifies a JWT token and returns the claims
pub fn verify_token(token: &str, secret: &str) -> Result<Claims, ServerError> {
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::new(Algorithm::HS256);
    
    let token_data = decode::<Claims>(token, &decoding_key, &validation)
        .map_err(|e| ServerError::Auth(format!("Invalid token: {}", e)))?;
    
    Ok(token_data.claims)
}

/// Creates a license token with subscription details
pub fn create_license_token(
    user: &User,
    secret: &str,
    subscription_type: &str,
    expiry_days: i64,
) -> Result<String, ServerError> {
    #[derive(Serialize)]
    struct LicenseClaims {
        sub: String,
        email: String,
        name: Option<String>,
        subscription_type: String,
        iat: i64,
        exp: i64,
    }

    let now = Utc::now();
    let iat = now.timestamp();
    let exp = (now + Duration::days(expiry_days)).timestamp();
    
    let claims = LicenseClaims {
        sub: user.id.to_string(),
        email: user.email.clone(),
        name: user.name.clone(),
        subscription_type: subscription_type.to_string(),
        iat,
        exp,
    };
    
    let header = Header::new(Algorithm::HS256);
    let encoding_key = EncodingKey::from_secret(secret.as_bytes());
    
    encode(&header, &claims, &encoding_key)
        .map_err(|e| ServerError::Internal(format!("Failed to create license token: {}", e)))
}

/// Middleware for requiring authentication
pub async fn require_auth(
    user: Result<AuthenticatedUser, ServerError>,
    request: Request,
    next: Next,
) -> Response {
    match user {
        Ok(user) => {
            let mut request = request;
            request.extensions_mut().insert(user);
            next.run(request).await
        }
        Err(err) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized",
                "message": err.to_string()
            }))
        ).into_response()
    }
}

/// Middleware for requiring subscription
pub async fn require_subscription(
    user: Result<AuthenticatedUser, ServerError>,
    request: Request,
    next: Next,
) -> Response {
    match user {
        Ok(user) => {
            if !user.has_subscription {
                return (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "error": "Forbidden",
                        "message": "Active subscription required"
                    }))
                ).into_response();
            }
            
            let mut request = request;
            request.extensions_mut().insert(user);
            next.run(request).await
        }
        Err(err) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Unauthorized",
                "message": err.to_string()
            }))
        ).into_response()
    }
}