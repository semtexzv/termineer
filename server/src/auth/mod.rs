//! Authentication module
//!
//! Handles user authentication with OAuth providers.

mod google;
mod routes;
pub mod session;

pub use routes::auth_routes;
