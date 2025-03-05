//! Server authentication module for license verification
//! 
//! This module handles communication with the AutoSWE server for
//! validating licenses and user authentication.

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// License verification request
#[derive(Serialize)]
pub struct VerifyLicenseRequest {
    pub license_key: String,
    pub client_id: String,
}

/// License verification response from server
#[derive(Deserialize, Debug)]
pub struct LicenseInfo {
    pub valid: bool,
    pub user_email: Option<String>,
    pub subscription_type: Option<String>,
    pub expires_at: Option<i64>,
    pub features: Vec<String>,
    pub message: Option<String>,
}

/// License verification client
pub struct LicenseClient {
    server_url: String,
    http_client: Client,
}

impl LicenseClient {
    /// Create a new license client
    pub fn new(server_url: String) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            server_url,
            http_client,
        }
    }
    
    /// Verify a license key with the server
    pub async fn verify_license(&self, license_key: &str) -> Result<LicenseInfo> {
        // Generate a unique client ID (could use a machine identifier in production)
        let client_id = format!("client_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
        
        // Create request payload
        let request = VerifyLicenseRequest {
            license_key: license_key.to_string(),
            client_id,
        };
        
        // Send request to server
        let response = self.http_client.post(format!("{}/license/verify", self.server_url))
            .json(&request)
            .send()
            .await
            .context("Failed to send license verification request")?;
            
        // Handle error status codes
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow::anyhow!("License verification failed: HTTP {}: {}", status, error_text));
        }
        
        // Parse response
        let license_info = response.json::<LicenseInfo>().await
            .context("Failed to parse license verification response")?;
            
        Ok(license_info)
    }
}

/// Check if a license is expired
pub fn is_license_expired(license_info: &LicenseInfo) -> bool {
    if let Some(expires_at) = license_info.expires_at {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        expires_at < now
    } else {
        // If no expiration is provided, consider it not expired
        false
    }
}