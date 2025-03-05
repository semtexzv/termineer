pub mod auth;
pub mod payment;
pub mod license;
pub mod mock_license;

use sqlx::SqlitePool;
use crate::config::Config;

/// Application state shared across handlers
pub struct AppState {
    pub db_pool: SqlitePool,
    pub config: Config,
}

pub use auth::*;
pub use payment::*;
pub use license::*;
pub use mock_license::*;