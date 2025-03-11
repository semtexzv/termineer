use serde::Deserialize;
use std::env;
use config::{Config as ConfigLib, ConfigError, Environment, File};

/// OAuth configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OAuthConfig {
    /// Google OAuth client ID
    pub google_client_id: Option<String>,
    /// Google OAuth client secret
    pub google_client_secret: Option<String>,
    /// Google OAuth redirect URI (optional, will be derived if not specified)
    pub google_redirect_uri: Option<String>,
}

/// Application configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Current environment (development, production)
    pub environment: String,
    /// Server host
    pub host: String,
    /// Server port
    pub port: u16,
    /// Database connection URL
    pub database_url: String,
    /// OAuth configuration
    #[serde(default)]
    pub oauth: OAuthConfig,
}

impl Config {
    /// Load configuration from environment variables and config files
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut builder = ConfigLib::builder()
            // Set default values
            .set_default("environment", "development")?
            .set_default("host", "127.0.0.1")?
            .set_default("port", 8080)?
            .set_default("database_url", "postgres://termineer:development@localhost:5432/termineer")?
            // OAuth defaults (these will be overridden by environment variables if provided)
            .set_default("oauth.google_client_id", "")?
            .set_default("oauth.google_client_secret", "")?;
            
        // Layer on the environment-specific values from config files if available
        if let Ok(env) = env::var("ENVIRONMENT") {
            builder = builder.add_source(File::with_name(&format!("config/{}", env)).required(false));
        }
        
        // Add settings from environment variables
        builder = builder.add_source(Environment::default().separator("__"));
        
        // Build and deserialize the config
        let config = builder.build()?;
        
        config.try_deserialize()
    }
    
    /// Check if running in development environment
    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
    
    /// Check if running in production environment
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}