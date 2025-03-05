//! Example client code for integrating with the AutoSWE server
//! 
//! This demonstrates how to verify licenses, check authentication,
//! and interact with the server API from the AutoSWE client.

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// License information structure
#[derive(Debug, Deserialize, Serialize, Clone)]
struct License {
    license_key: String,
    expires_at: Option<String>,
    user_email: Option<String>,
}

/// Response from license verification API
#[derive(Debug, Deserialize)]
struct VerifyResponse {
    valid: bool,
    expires_at: Option<String>,
    user_email: Option<String>,
    subscription_type: Option<String>,
    message: Option<String>,
}

/// Main entry point showing how to integrate license checking in AutoSWE
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Print example header
    println!("AutoSWE License Verification Example");
    println!("===================================");
    println!();
    
    // Configuration
    let server_url = env::var("AUTOSWE_SERVER_URL")
        .unwrap_or_else(|_| "https://api.autoswe.com".to_string());
    
    let license_file = get_license_file_path()?;
    let http_client = Client::new();
    
    // First, check for an existing license
    let result = if license_file.exists() {
        println!("Found existing license file");
        
        // Load and parse the license file
        let license_data = fs::read_to_string(&license_file)?;
        let license: License = serde_json::from_str(&license_data)?;
        
        // Check if the license is expired based on the expiration date
        if let Some(expires_str) = &license.expires_at {
            if is_expired(expires_str) {
                println!("License has expired. Verifying with server...");
                verify_with_server(&http_client, &server_url, &license.license_key).await?
            } else {
                println!("License is valid and not expired.");
                true
            }
        } else {
            // If no expiry info, verify with server
            println!("License missing expiration date. Verifying with server...");
            verify_with_server(&http_client, &server_url, &license.license_key).await?
        }
    } else {
        println!("No license file found. Please log in and subscribe.");
        false
    };
    
    if result {
        println!("\nLicense validation successful!");
        println!("Starting AutoSWE with full functionality...");
        
        // Here you would continue with the normal operation of AutoSWE
        run_autoswe().await?;
    } else {
        println!("\nNo valid license found. Running in limited mode.");
        println!("Please subscribe at https://autoswe.com to unlock full functionality.");
        
        // Demonstrate the login flow
        if prompt_for_login() {
            // This would normally open a browser for OAuth login
            println!("Opening browser for login...");
            println!("After login and subscription, restart AutoSWE to activate.");
        } else {
            // Run in limited mode
            run_autoswe_limited().await?;
        }
    }
    
    Ok(())
}

/// Get the path to the license file
fn get_license_file_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config_dir = if cfg!(windows) {
        // Windows: %APPDATA%\AutoSWE
        let app_data = env::var("APPDATA")?;
        PathBuf::from(app_data).join("AutoSWE")
    } else if cfg!(target_os = "macos") {
        // macOS: ~/Library/Application Support/AutoSWE
        let home = env::var("HOME")?;
        PathBuf::from(home).join("Library/Application Support/AutoSWE")
    } else {
        // Linux: ~/.config/autoswe
        let home = env::var("HOME")?;
        PathBuf::from(home).join(".config/autoswe")
    };
    
    // Create the directory if it doesn't exist
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)?;
    }
    
    Ok(config_dir.join("license.json"))
}

/// Check if a license is expired based on the expiration date string
fn is_expired(expires_at: &str) -> bool {
    // Parse the expiration date
    if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(expires_at) {
        let now = chrono::Utc::now();
        now > expires
    } else {
        // If we can't parse the date, assume it's expired to be safe
        true
    }
}

/// Verify a license key with the server
async fn verify_with_server(
    client: &Client, 
    server_url: &str, 
    license_key: &str
) -> Result<bool, Box<dyn std::error::Error>> {
    // Prepare the request body
    let body = serde_json::json!({
        "license_key": license_key,
        "client_version": env!("CARGO_PKG_VERSION"),
        "client_platform": std::env::consts::OS
    });
    
    // Send the verification request
    let response = client
        .post(&format!("{}/license/verify", server_url))
        .json(&body)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    
    if response.status() != StatusCode::OK {
        println!("Server error: {}", response.status());
        return Ok(false);
    }
    
    let verify_result: VerifyResponse = response.json().await?;
    
    if let Some(msg) = verify_result.message {
        println!("Server message: {}", msg);
    }
    
    Ok(verify_result.valid)
}

/// Prompt the user to log in
fn prompt_for_login() -> bool {
    println!("\nWould you like to log in and subscribe now? (y/n)");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or(0);
    input.trim().to_lowercase().starts_with('y')
}

/// Simulate running AutoSWE with full functionality
async fn run_autoswe() -> Result<(), Box<dyn std::error::Error>> {
    println!("AutoSWE is now running with full functionality");
    println!("- Access to all tools enabled");
    println!("- Premium models available");
    println!("- Advanced features unlocked");
    
    // Simulate some activity
    for i in 1..=5 {
        sleep(Duration::from_millis(500)).await;
        println!("  processing... {}/5", i);
    }
    
    println!("Ready!");
    Ok(())
}

/// Simulate running AutoSWE in limited mode
async fn run_autoswe_limited() -> Result<(), Box<dyn std::error::Error>> {
    println!("AutoSWE is running in limited mode");
    println!("- Basic tools only");
    println!("- Limited model access");
    println!("- Advanced features unavailable");
    
    // Simulate some activity
    for i in 1..=3 {
        sleep(Duration::from_millis(500)).await;
        println!("  processing... {}/3", i);
    }
    
    println!("Ready (limited functionality)");
    Ok(())
}