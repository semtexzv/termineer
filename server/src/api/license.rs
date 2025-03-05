use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use log::{info, error};

use crate::auth::jwt::{AuthenticatedUser, verify_token as jwt_verify_token};
use crate::AppState;
use crate::db::operations::{LicenseOps, UserOps};
use crate::errors::ServerError;

/// Request for verifying a license key
#[derive(Debug, Deserialize)]
pub struct VerifyLicenseRequest {
    license_key: String,
    client_version: Option<String>,
    client_platform: Option<String>,
}

/// Response for license verification
#[derive(Debug, Serialize)]
pub struct VerifyLicenseResponse {
    valid: bool,
    expires_at: Option<DateTime<Utc>>,
    user_email: Option<String>,
    subscription_type: Option<String>,
    message: Option<String>,
}

/// Verify a license key
pub async fn verify_license(
    Json(req): Json<VerifyLicenseRequest>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    let license_key = &req.license_key;
    
    // First, try to decode and verify the JWT token itself
    let decoded = match jwt_verify_token(license_key, &state.config.jwt_secret) {
        Ok(claims) => claims,
        Err(e) => {
            info!("Invalid license key format: {}", e);
            return Ok((StatusCode::OK, Json(VerifyLicenseResponse {
                valid: false,
                expires_at: None,
                user_email: None,
                subscription_type: None,
                message: Some("Invalid license key".to_string()),
            })));
        }
    };
    
    // If token is valid, check if it exists in our database
    match LicenseOps::verify(&state.db_pool, license_key).await {
        Ok(Some(license)) => {
            // Get the user information
            let user = UserOps::find_by_id(&state.db_pool, license.user_id).await?
                .ok_or_else(|| ServerError::NotFound("User not found".to_string()))?;
            
            // If the user has an active subscription, the license is valid
            if user.has_subscription {
                info!("Valid license verified for user: {}", user.email);
                
                // TODO: Record usage statistics if needed
                
                Ok((StatusCode::OK, Json(VerifyLicenseResponse {
                    valid: true,
                    expires_at: Some(license.expires_at),
                    user_email: Some(user.email),
                    subscription_type: Some(decoded.sub),
                    message: None,
                })))
            } else {
                info!("License found but subscription inactive for user: {}", user.email);
                Ok((StatusCode::OK, Json(VerifyLicenseResponse {
                    valid: false,
                    expires_at: Some(license.expires_at),
                    user_email: Some(user.email),
                    subscription_type: None,
                    message: Some("Subscription is no longer active".to_string()),
                })))
            }
        },
        Ok(None) => {
            info!("License key not found in database or expired");
            Ok((StatusCode::OK, Json(VerifyLicenseResponse {
                valid: false,
                expires_at: None,
                user_email: None,
                subscription_type: None,
                message: Some("License key not found or expired".to_string()),
            })))
        },
        Err(e) => {
            error!("Database error verifying license: {}", e);
            Err(e)
        }
    }
}

/// Get license details for the authenticated user
pub async fn get_license_details(
    auth_user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    #[derive(Serialize)]
    struct LicenseDetails {
        license_key: Option<String>,
        expires_at: Option<DateTime<Utc>>,
        status: String,
        subscription_type: Option<String>,
    }
    
    // Find the active license for this user
    let license = LicenseOps::find_by_user_id(&state.db_pool, auth_user.user_id).await?;
    
    let response = match license {
        Some(license) if license.is_active && auth_user.has_subscription => {
            // Find subscription details to get the plan type
            let subscription = crate::db::operations::SubscriptionOps::find_by_user_id(
                &state.db_pool, auth_user.user_id
            ).await?;
            
            let subscription_type = subscription.map(|s| s.plan_id);
            
            LicenseDetails {
                license_key: Some(license.license_key),
                expires_at: Some(license.expires_at),
                status: "active".to_string(),
                subscription_type,
            }
        },
        Some(license) if license.is_active => {
            // License exists but subscription is inactive
            LicenseDetails {
                license_key: Some(license.license_key),
                expires_at: Some(license.expires_at),
                status: "inactive".to_string(),
                subscription_type: None,
            }
        },
        _ => {
            // No active license
            LicenseDetails {
                license_key: None,
                expires_at: None,
                status: "none".to_string(),
                subscription_type: None,
            }
        }
    };
    
    Ok((StatusCode::OK, Json(response)))
}