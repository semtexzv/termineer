use serde::Deserialize;
use std::env;
use config::{Config as ConfigLib, ConfigError, Environment, File};

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
}

impl Config {
    /// Load configuration from environment variables and config files
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut builder = ConfigLib::builder()
            // Set default values
            .set_default("environment", "development")?
            .set_default("host", "127.0.0.1")?
            .set_default("port", 8080)?
            .set_default("database_url", "postgres://termineer:development@localhost:5432/termineer")?;
            
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