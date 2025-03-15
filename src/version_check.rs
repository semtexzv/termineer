//! Version check module
//!
//! This module provides functionality to check for newer versions of Termineer
//! by querying the NPM registry.

use anyhow::Result;
use serde::Deserialize;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;

// Cache for version check results to avoid hammering the NPM registry
lazy_static! {
    static ref VERSION_CACHE: Arc<Mutex<Option<(String, Instant)>>> = Arc::new(Mutex::new(None));
    static ref CACHE_DURATION: Duration = Duration::from_secs(3600); // Cache for 1 hour
}

// The current version from Cargo.toml
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize, Debug)]
struct NpmPackageInfo {
    #[serde(rename = "dist-tags")]
    dist_tags: DistTags,
}

#[derive(Deserialize, Debug)]
struct DistTags {
    latest: String,
}

/// Check if a newer version is available
///
/// Contacts the NPM registry to check if there's a newer version of Termineer available.
/// Uses caching to avoid repeated API calls.
///
/// Returns a tuple with:
/// - Boolean: true if update is available
/// - String: latest version if available, current version otherwise
/// - String: optional update message
pub async fn check_for_updates() -> Result<(bool, String, Option<String>)> {
    // Check if we have a cached result
    {
        let cache = VERSION_CACHE.lock().unwrap();
        if let Some((latest_version, cache_time)) = &*cache {
            if cache_time.elapsed() < *CACHE_DURATION {
                let has_update = is_newer_version(latest_version, CURRENT_VERSION);
                let message = if has_update {
                    Some(format!("🔄 Update available: {} → {}\nRun 'npm update -g termineer' to update", 
                                 CURRENT_VERSION, latest_version))
                } else {
                    None
                };
                return Ok((has_update, latest_version.clone(), message));
            }
        }
    }
    
    // Cache expired or not found, fetch the latest version from NPM
    match fetch_latest_version().await {
        Ok(latest_version) => {
            // Update the cache
            {
                let mut cache = VERSION_CACHE.lock().unwrap();
                *cache = Some((latest_version.clone(), Instant::now()));
            }
            
            let has_update = is_newer_version(&latest_version, CURRENT_VERSION);
            let message = if has_update {
                Some(format!("🔄 Update available: {} → {}\nRun 'npm update -g termineer' to update", 
                             CURRENT_VERSION, latest_version))
            } else {
                None
            };
            
            Ok((has_update, latest_version, message))
        },
        Err(e) => {
            // Error fetching version, return current version with no update notification
            // Using our own logging system instead of log crate
            crate::bprintln!(dev: "Failed to check for updates: {}", e);
            Ok((false, CURRENT_VERSION.to_string(), None))
        }
    }
}

/// Fetch the latest version from NPM registry
async fn fetch_latest_version() -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    
    let resp = client
        .get("https://registry.npmjs.org/termineer")
        .header("User-Agent", format!("termineer/{}", CURRENT_VERSION))
        .send()
        .await?;
    
    if resp.status().is_success() {
        let package_info: NpmPackageInfo = resp.json().await?;
        Ok(package_info.dist_tags.latest)
    } else {
        Err(anyhow::anyhow!("Failed to fetch package info: HTTP {}", resp.status()))
    }
}

/// Compare version strings to determine if latest is newer than current
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
         .split('.')
         .filter_map(|s| s.parse::<u32>().ok())
         .collect()
    };
    
    let latest_parts = parse_version(latest);
    let current_parts = parse_version(current);
    
    for i in 0..std::cmp::min(latest_parts.len(), current_parts.len()) {
        if latest_parts[i] > current_parts[i] {
            return true;
        } else if latest_parts[i] < current_parts[i] {
            return false;
        }
    }
    
    // If we got here and have different lengths, the longer one with same prefix is newer
    latest_parts.len() > current_parts.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.2.0", "0.1.0"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(is_newer_version("0.2.1", "0.2.0"));
        assert!(is_newer_version("0.10.0", "0.9.0"));
        assert!(is_newer_version("0.9.10", "0.9.9"));
        assert!(is_newer_version("1.0.0", "0.9.0"));
        assert!(is_newer_version("v1.0.0", "0.9.0"));
        assert!(is_newer_version("1.0.0", "v0.9.0"));
        
        assert!(!is_newer_version("0.1.0", "0.2.0"));
        assert!(!is_newer_version("0.9.9", "1.0.0"));
        assert!(!is_newer_version("0.9.0", "0.10.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("v1.0.0", "1.0.0"));
    }
}