//! Authentication module for Termineer
//!
//! This module provides authentication-related functionality, including:
//! - OAuth-based login flow with browser integration
//! - Token storage and retrieval
//! - User information management
//! - Subscription level handling

// Re-export authentication types and functions
mod client;
pub mod functions;

// Only export what's actually used by the main crate
pub use functions::{authenticate_user, attempt_cached_auth, AuthConfig};