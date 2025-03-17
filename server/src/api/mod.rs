//! API module
//!
//! Handles API endpoints for the application.

pub mod auth;

use crate::config::Config;
use sqlx::PgPool;

/// Application state shared across handlers
pub struct AppState {
    pub db_pool: PgPool,
    pub config: Config,
}
