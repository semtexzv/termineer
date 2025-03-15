//! Authentication utility functions for Termineer
//! This file provides implementations for the authentication-related functions
//! used throughout the application

use super::client::{is_subscription_expired, AuthClient};
use crossterm::{
    cursor::MoveToNextLine,
    execute,
    style::{Color, ResetColor, SetForegroundColor},
};
use std::io::stdout;

/// Configuration struct passed to authentication functions
/// This allows us to avoid circular dependencies
pub struct AuthConfig {
    pub user_email: Option<String>,
    pub subscription_type: Option<String>,
    pub app_mode: String, // Use string instead of enum to avoid circular dependencies
}

// No longer using types module directly

/// Callback function type for setting app mode
pub type SetAppModeCallback = fn(String);

/// Try to load authentication from a previous session without user interaction
pub async fn attempt_cached_auth(
    config: &mut AuthConfig,
    set_app_mode: SetAppModeCallback,
) -> anyhow::Result<()> {
    // Create the auth client with the server URL
    let auth_client = AuthClient::new(obfstr::obfstring!("https://termineer.io").to_string());

    // Debug info for token loading
    println!("Attempting to load cached auth token...");
    
    // Try to get user info from saved credentials
    match auth_client.get_cached_user_info().await {
        Ok(user_info) => {
            // Check if subscription is expired
            if is_subscription_expired(&user_info) {
                return Err("Your subscription has expired. Please log in again.".into());
            }

            // Authentication successful, set user information
            config.user_email = Some(user_info.email.clone());
            if let Some(subscription) = user_info.subscription_type.clone() {
                config.subscription_type = Some(subscription);
            }

            // Get the subscription level as a string
            let subscription_level = user_info.subscription_type
                .as_deref()
                .unwrap_or("free")
                .to_lowercase();
            
            // Use that as the app mode
            let app_mode = subscription_level.clone();

            // Update the app mode using the callback
            set_app_mode(app_mode.clone());
            config.app_mode = app_mode;

            // Quietly indicate the mode (no big banners)
            println!(
                "✓ Authenticated as {} ({})",
                user_info.email,
                config.app_mode.to_uppercase()
            );

            Ok(())
        }
        Err(e) => {
            // No valid cached credentials found
            Err(format!("No valid saved credentials: {}", e).into())
        }
    }
}

/// Authenticate user with the server using OAuth
pub async fn authenticate_user(
    config: &mut AuthConfig,
    set_app_mode: SetAppModeCallback,
) -> anyhow::Result<()> {
    // Initialize auth client
    let auth_client = AuthClient::new(obfstr::obfstring!("https://termineer.io").to_string());

    // Start OAuth flow
    println!("Starting authentication flow...");
    println!("This will open your browser to authenticate with your account.");
    println!("If you don't have an account, you can create one during this process.");

    // Perform OAuth authentication
    let user_info = match auth_client.authenticate().await {
        Ok(info) => info,
        Err(e) => {
            // Print error in red
            execute!(stdout(), SetForegroundColor(Color::Red), MoveToNextLine(1),).ok();
            println!("❌ Authentication failed: {}", e);
            execute!(stdout(), ResetColor).ok();

            return Err(format!("Authentication error: {}", e).into());
        }
    };

    // Check if subscription is expired
    if is_subscription_expired(&user_info) {
        // Print error in yellow
        execute!(
            stdout(),
            SetForegroundColor(Color::Yellow),
            MoveToNextLine(1),
        )
        .ok();
        println!("⚠️ Your subscription has expired. Please renew your subscription.");
        execute!(stdout(), ResetColor).ok();

        return Err("Your subscription has expired. Please renew your subscription.".into());
    }

    // Log successful authentication with green text
    execute!(
        stdout(),
        SetForegroundColor(Color::Green),
        MoveToNextLine(1),
    )
    .ok();
    println!("✅ Authentication successful for: {}", user_info.email);

    if let Some(subscription) = &user_info.subscription_type {
        println!("📋 Subscription: {}", subscription);
    }

    execute!(stdout(), ResetColor).ok();

    // Save user information for future use
    config.user_email = Some(user_info.email.clone());
    if let Some(subscription) = user_info.subscription_type.clone() {
        config.subscription_type = Some(subscription.clone());
    }

    // Get the subscription level as a string
    let subscription_level = user_info.subscription_type
        .as_deref()
        .unwrap_or("free")
        .to_lowercase();
    
    // Use that as the app mode
    let app_mode = subscription_level.clone();

    // Update the app mode using the callback
    set_app_mode(app_mode.clone());
    config.app_mode = app_mode;

    // Display the mode
    println!("🔑 Access Level: {}", config.app_mode.to_uppercase());

    // Optional: Add a small delay to ensure the user sees the verification message
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

    Ok(())
}