//! Mock license verification for local testing
//!
//! This module provides simplified license verification for local development and testing,
//! without requiring JWT token validation or database access.

use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};
use log::{info, error};

use crate::AppState;
use crate::errors::ServerError;

/// Request for verifying a license key
#[derive(Debug, Deserialize)]
pub struct VerifyLicenseRequest {
    pub license_key: String,
    pub client_id: String,
}

/// Response for license verification
#[derive(Debug, Serialize)]
pub struct LicenseVerifyResponse {
    pub valid: bool,
    pub user_email: Option<String>,
    pub subscription_type: Option<String>,
    pub expires_at: Option<i64>,
    pub features: Vec<String>,
    pub message: Option<String>,
}

/// Valid test license keys with associated data
const TEST_LICENSES: [(&str, &str, &str, bool); 3] = [
    // (key, email, subscription_type, is_valid)
    ("TEST-DEV-LICENSE-KEY", "developer@example.com", "developer", true),
    ("TEST-PRO-LICENSE-KEY", "pro@example.com", "professional", true),
    ("TEST-EXPIRED-LICENSE", "expired@example.com", "basic", false),
];

/// Verify a license key (mock implementation for testing)
pub async fn verify_license(
    Json(req): Json<VerifyLicenseRequest>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    info!("Verifying license key: {}", req.license_key);
    
    // For testing, accept specific test license keys
    let now = Utc::now();
    let mut response = LicenseVerifyResponse {
        valid: false,
        user_email: None,
        subscription_type: None,
        expires_at: None,
        features: Vec::new(),
        message: Some("Invalid license key".to_string()),
    };
    
    for (key, email, subscription, is_valid) in TEST_LICENSES.iter() {
        if &req.license_key == key {
            // Calculate expiration date based on validity
            let expires_at = if *is_valid {
                // Valid keys expire 1 year from now
                now + Duration::days(365)
            } else {
                // Expired keys expired 30 days ago
                now - Duration::days(30)
            };
            
            response = LicenseVerifyResponse {
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
    
    // If using DEV-MODE flag, consider any key valid for testing
    if !response.valid && req.license_key.starts_with("DEV-") {
        response = LicenseVerifyResponse {
            valid: true,
            user_email: Some("developer@localhost".to_string()),
            subscription_type: Some("development".to_string()),
            expires_at: Some((now + Duration::days(30)).timestamp()),
            features: vec!["all".to_string()],
            message: None,
        };
    }
    
    info!("License verification result: {}", response.valid);
    Ok((StatusCode::OK, Json(response)))
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