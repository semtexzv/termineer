use serde::Deserialize;
use std::env;
use config::{Config as ConfigLib, ConfigError, Environment, File};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub environment: String,
    pub host: String,
    pub port: u16,
    pub database_url: String,
    
    // OAuth configuration
    pub google_client_id: String,
    pub google_client_secret: String,
    pub oauth_redirect_url: String,
    
    // Stripe configuration
    pub stripe_secret_key: String,
    pub stripe_webhook_secret: String,
    
    // JWT configuration
    pub jwt_secret: String,
    pub jwt_expiry: i64,
    
    // Application URLs
    pub frontend_url: String,
    pub success_redirect_url: String,
    pub cancel_redirect_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut builder = ConfigLib::builder()
            // Start with default values
            .set_default("environment", "development")?
            .set_default("host", "127.0.0.1")?
            .set_default("port", 8080)?
            .set_default("jwt_expiry", 86400)?
            // Set default values for SQLite
            .set_default("database_url", "sqlite:data/autoswe.db")?
            // For test/development, use mock values
            .set_default("google_client_id", "mock_client_id")?
            .set_default("google_client_secret", "mock_client_secret")?
            .set_default("oauth_redirect_url", "http://localhost:3000/auth/google/callback")?
            .set_default("jwt_secret", "development_jwt_secret_key")?
            .set_default("stripe_secret_key", "mock_stripe_key")?
            .set_default("stripe_webhook_secret", "mock_webhook_secret")?
            .set_default("frontend_url", "http://localhost:8732")?
            .set_default("success_redirect_url", "http://localhost:8732/payment/success")?
            .set_default("cancel_redirect_url", "http://localhost:8732/payment/cancel")?;
            
        // Layer on the environment-specific values
        if let Ok(env) = env::var("ENVIRONMENT") {
            builder = builder.add_source(File::with_name(&format!("config/{}", env)).required(false));
        }
        
        // Add in settings from environment variables
        builder = builder.add_source(Environment::default().separator("__"));
        
        // Build and deserialize the config
        let config = builder.build()?;
        
        config.try_deserialize()
    }
    
    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
    
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }
}