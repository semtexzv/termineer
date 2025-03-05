//! Protected prompts module
//!
//! This module handles encrypted prompt templates, providing
//! secure storage and access with runtime decryption.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::{Mutex, Once};

// Include the auto-generated encrypted prompts data
include!(concat!(env!("OUT_DIR"), "/encrypted_prompts.rs"));

// Include the encryption key directly from the binary file generated during build
static ENCRYPTION_KEY_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/encryption_key.bin"));

// Cache for decrypted templates to avoid repeated decryption
lazy_static! {
    static ref DECRYPTED_CACHE: Mutex<HashMap<String, String>> = Mutex::new(HashMap::new());
    static ref INIT: Once = Once::new();
}

/// Initialize the protected prompts module
fn initialize() {
    INIT.call_once(|| {
        // Any one-time initialization would go here
    });
}

/// Get a decrypted prompt template by name
///
/// This function retrieves a prompt template, decrypts it,
/// and returns the decrypted content.
pub fn get_prompt_template(name: &str) -> Option<String> {
    initialize();

    // Append .hbs if needed
    let file_path = if !name.ends_with(".hbs") {
        format!("{}.hbs", name)
    } else {
        name.to_string()
    };
    
    // Normalize path to use forward slashes
    let normalized_path = file_path.replace('\\', "/");
    
    // Check cache first
    {
        let cache = DECRYPTED_CACHE.lock().unwrap();
        // Remove .hbs for cache lookup
        let cache_key = normalized_path.strip_suffix(".hbs").unwrap_or(&normalized_path).to_string();
        if let Some(cached) = cache.get(&cache_key) {
            return Some(cached.clone());
        }
    }

    // Find the encrypted data in the ENCRYPTED_PROMPTS array
    // We need to handle the case where the normalized_path doesn't have .hbs extension
    let encrypted_content = if normalized_path.ends_with(".hbs") {
        // If path already has .hbs, look it up directly
        ENCRYPTED_PROMPTS
            .iter()
            .find(|(path, _)| *path == normalized_path)
            .map(|(_, data)| *data)
    } else {
        // If path doesn't have .hbs, append it for lookup
        let path_with_extension = format!("{}.hbs", normalized_path);
        ENCRYPTED_PROMPTS
            .iter()
            .find(|(path, _)| *path == path_with_extension)
            .map(|(_, data)| *data)
    };
    
    let encrypted_content = match encrypted_content {
        Some(content) => content,
        None => return None, // File not found
    };

    // Decrypt the content
    if encrypted_content.len() <= 12 {
        return None; // Too short to contain nonce + data
    }

    // Extract nonce from beginning of data
    let nonce_bytes = &encrypted_content[0..12];
    let nonce = Nonce::from_slice(nonce_bytes);
    let ciphertext = &encrypted_content[12..];

    // Use the encryption key bytes directly
    let key = Key::<Aes256Gcm>::from_slice(ENCRYPTION_KEY_BYTES);
    let cipher = Aes256Gcm::new(key);

    // Decrypt
    let decrypted = match cipher.decrypt(nonce, ciphertext) {
        Ok(data) => data,
        Err(_) => return None, // Decryption failed
    };

    // Convert to string
    let template_str = match String::from_utf8(decrypted) {
        Ok(s) => s,
        Err(_) => return None, // Invalid UTF-8
    };

    // Store in cache (without .hbs extension)
    {
        let mut cache = DECRYPTED_CACHE.lock().unwrap();
        let cache_key = normalized_path.strip_suffix(".hbs").unwrap_or(&normalized_path).to_string();
        cache.insert(cache_key, template_str.clone());
    }

    Some(template_str)
}

/// Get all available template names
pub fn list_available_templates() -> Vec<String> {
    // Extract all template names from the ENCRYPTED_PROMPTS array
    ENCRYPTED_PROMPTS
        .iter()
        .filter_map(|(path, _)| {
            // Only include .hbs files
            if path.ends_with(".hbs") {
                // Remove the .hbs extension for the template name
                Some(path.strip_suffix(".hbs").unwrap_or(path).to_string())
            } else {
                None
            }
        })
        .collect()
}