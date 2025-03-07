use sqlx::PgPool;
use crate::config::Config;

/// Application state shared across handlers
pub struct AppState {
    pub db_pool: PgPool,
    pub config: Config,
}