//! Mock server authentication module for testing
//! 
//! This simplified version bypasses the browser-based authentication flow
//! and uses a direct mock token for testing purposes.

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// User information response from server
#[derive(Deserialize, Debug)]
pub struct UserInfo {
    pub email: String,
    pub display_name: Option<String>,
    pub subscription_type: Option<String>,
    pub subscription_status: Option<String>,
    pub expires_at: Option<i64>,
    pub features: Vec<String>,
}

/// Authentication client (simplified mock version)
pub struct AuthClient {
    http_client: Client,
}

impl AuthClient {
    /// Create a new authentication client
    pub fn new(_server_url: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            http_client,
        }
    }
    
    /// Authenticate user with completely mocked flow (no network needed)
    pub async fn authenticate(&self) -> Result<UserInfo> {
        println!("Simulating OAuth authentication flow with mock data");
        println!("No server connection required - using mock data directly");
        
        // Skip all server interaction and return simulated user info immediately
        println!("Authentication successful! Using mock user data for testing");
        Ok(UserInfo {
            email: "test@example.com".to_string(),
            display_name: Some("Test User".to_string()),
            subscription_type: Some("premium".to_string()),
            subscription_status: Some("active".to_string()),
            expires_at: Some(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64 + 30 * 86400),
            features: vec!["all".to_string()],
        })
    }
}

/// Check if a subscription is expired
pub fn is_subscription_expired(user_info: &UserInfo) -> bool {
    if let Some(expires_at) = user_info.expires_at {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        expires_at < now
    } else {
        // If no expiration is provided, consider it not expired
        false
    }
}