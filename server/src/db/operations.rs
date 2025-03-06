use chrono::Utc;
use sqlx::{PgPool, Error as SqlxError};
use uuid::Uuid;
use log::{info, error};

use crate::db::models::{User, Subscription, LicenseKey, SubscriptionStatus};
use crate::errors::ServerError;

/// Operations for User model
pub struct UserOps;

impl UserOps {
    /// Find a user by ID
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, ServerError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Database error finding user by ID: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        Ok(user)
    }
    
    /// Find a user by email
    pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, ServerError> {
        let user = sqlx::query_as::<_, User>(
            "SELECT * FROM users WHERE email = $1"
        )
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Database error finding user by email: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        Ok(user)
    }
    
    /// Create a new user
    pub async fn create(
        pool: &PgPool,
        email: &str,
        name: Option<String>,
        auth_provider: String,
        auth_provider_id: Option<String>,
    ) -> Result<User, ServerError> {
        let now = Utc::now();
        
        let user = sqlx::query_as::<_, User>(
            r#"
            INSERT INTO users (
                id, email, name, auth_provider, auth_provider_id, 
                is_active, has_subscription, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(email)
        .bind(name)
        .bind(auth_provider)
        .bind(auth_provider_id)
        .bind(true)  // is_active
        .bind(false) // has_subscription
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Database error creating user: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Created new user: {}", user.email);
        Ok(user)
    }
    
    /// Find or create a user from OAuth authentication
    pub async fn find_or_create_from_oauth(
        pool: &PgPool,
        email: &str,
        name: Option<String>,
        auth_provider: String,
    ) -> Result<User, ServerError> {
        // Try to find an existing user
        if let Some(user) = Self::find_by_email(pool, email).await? {
            // Update the user's auth provider if needed
            if user.auth_provider != auth_provider {
                return Self::update_auth_provider(pool, user.id, auth_provider).await;
            }
            return Ok(user);
        }
        
        // Create a new user if not found
        Self::create(pool, email, name, auth_provider, None).await
    }
    
    /// Find or create a user from email only (for payment processing)
    pub async fn find_or_create_from_email(
        pool: &PgPool,
        email: &str,
        name: Option<String>,
    ) -> Result<User, ServerError> {
        // Try to find an existing user
        if let Some(user) = Self::find_by_email(pool, email).await? {
            return Ok(user);
        }
        
        // Create a new user if not found - use "stripe" as auth provider
        Self::create(pool, email, name, "stripe".to_string(), None).await
    }
    
    /// Update a user's authentication provider
    pub async fn update_auth_provider(
        pool: &PgPool,
        user_id: Uuid,
        auth_provider: String,
    ) -> Result<User, ServerError> {
        let now = Utc::now();
        
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET auth_provider = $1, updated_at = $2
            WHERE id = $3
            RETURNING *
            "#
        )
        .bind(auth_provider)
        .bind(now)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Database error updating user auth provider: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Updated auth provider for user: {}", user.email);
        Ok(user)
    }
    
    /// Update a user's subscription status
    pub async fn update_subscription_status(
        pool: &PgPool,
        user_id: Uuid,
        has_subscription: bool,
    ) -> Result<User, ServerError> {
        let now = Utc::now();
        
        let user = sqlx::query_as::<_, User>(
            r#"
            UPDATE users
            SET has_subscription = $1, updated_at = $2
            WHERE id = $3
            RETURNING *
            "#
        )
        .bind(has_subscription)
        .bind(now)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Database error updating user subscription status: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Updated subscription status for user: {}", user.email);
        Ok(user)
    }
}

/// Operations for Subscription model
pub struct SubscriptionOps;

impl SubscriptionOps {
    /// Find a subscription by user ID
    pub async fn find_by_user_id(pool: &PgPool, user_id: Uuid) -> Result<Option<Subscription>, ServerError> {
        let subscription = sqlx::query_as::<_, Subscription>(
            "SELECT * FROM subscriptions WHERE user_id = $1 ORDER BY created_at DESC LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Database error finding subscription: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        Ok(subscription)
    }
    
    /// Find a subscription by Stripe subscription ID
    pub async fn find_by_stripe_subscription_id(
        pool: &PgPool,
        stripe_subscription_id: &str,
    ) -> Result<Option<Subscription>, ServerError> {
        let subscription = sqlx::query_as::<_, Subscription>(
            "SELECT * FROM subscriptions WHERE stripe_subscription_id = $1"
        )
        .bind(stripe_subscription_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Database error finding subscription by Stripe ID: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        Ok(subscription)
    }
    
    /// Create a new subscription
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        stripe_customer_id: String,
        stripe_subscription_id: String,
        plan_id: &str,
        status: SubscriptionStatus,
        current_period_start: chrono::DateTime<Utc>,
        current_period_end: chrono::DateTime<Utc>,
        cancel_at_period_end: bool,
    ) -> Result<Subscription, ServerError> {
        let now = Utc::now();
        
        // Begin a transaction
        let mut tx = pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        // Create the subscription
        let subscription = sqlx::query_as::<_, Subscription>(
            r#"
            INSERT INTO subscriptions (
                id, user_id, stripe_customer_id, stripe_subscription_id, plan_id,
                status, current_period_start, current_period_end, cancel_at_period_end,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(stripe_customer_id)
        .bind(stripe_subscription_id)
        .bind(plan_id)
        .bind(&status)
        .bind(current_period_start)
        .bind(current_period_end)
        .bind(cancel_at_period_end)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            error!("Database error creating subscription: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        // Update the user's subscription status
        sqlx::query(
            r#"
            UPDATE users
            SET has_subscription = true, updated_at = $1
            WHERE id = $2
            "#
        )
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Database error updating user subscription flag: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        // Commit the transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Created new subscription for user: {}", user_id);
        Ok(subscription)
    }
    
    /// Update a subscription's status
    pub async fn update_status(
        pool: &PgPool,
        subscription_id: Uuid,
        status: SubscriptionStatus,
        current_period_end: Option<chrono::DateTime<Utc>>,
        cancel_at_period_end: bool,
    ) -> Result<Subscription, ServerError> {
        let now = Utc::now();
        
        // Begin transaction
        let mut tx = pool.begin().await.map_err(|e| {
            error!("Failed to start transaction: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        // Update the subscription
        let mut query = "UPDATE subscriptions SET status = $1, updated_at = $2".to_string();
        let mut param_index = 3;
        
        if let Some(_) = current_period_end {
            query.push_str(&format!(", current_period_end = ${}", param_index));
            param_index += 1;
        }
        
        query.push_str(&format!(", cancel_at_period_end = ${}", param_index));
        param_index += 1;
        
        query.push_str(&format!(" WHERE id = ${} RETURNING *", param_index));
        
        let mut query_builder = sqlx::query_as::<_, Subscription>(&query)
            .bind(&status)
            .bind(now);
        
        if let Some(end) = current_period_end {
            query_builder = query_builder.bind(end);
        }
        
        query_builder = query_builder.bind(cancel_at_period_end);
        query_builder = query_builder.bind(subscription_id);
        
        let subscription = query_builder
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| {
                error!("Database error updating subscription: {}", e);
                ServerError::Database(e.to_string())
            })?;
        
        // Make a copy of the status to determine active subscription state
        let status_copy = status.clone();
        
        // Update the user's subscription status if needed
        let has_active_subscription = match status_copy {
            SubscriptionStatus::Active | SubscriptionStatus::Trialing => true,
            _ => false,
        };
        
        sqlx::query(
            r#"
            UPDATE users
            SET has_subscription = $1, updated_at = $2
            WHERE id = $3
            "#
        )
        .bind(has_active_subscription)
        .bind(now)
        .bind(subscription.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            error!("Database error updating user subscription flag: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        // Commit the transaction
        tx.commit().await.map_err(|e| {
            error!("Failed to commit transaction: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Updated subscription status to {:?} for user: {}", status, subscription.user_id);
        Ok(subscription)
    }
}

/// Operations for LicenseKey model
pub struct LicenseOps;

impl LicenseOps {
    /// Find a license by user ID
    pub async fn find_by_user_id(pool: &PgPool, user_id: Uuid) -> Result<Option<LicenseKey>, ServerError> {
        let license = sqlx::query_as::<_, LicenseKey>(
            "SELECT * FROM license_keys WHERE user_id = $1 AND is_active = true ORDER BY created_at DESC LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Database error finding license: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        Ok(license)
    }
    
    /// Create a new license key
    pub async fn create(
        pool: &PgPool,
        user_id: Uuid,
        license_key: &str,
        expires_at: chrono::DateTime<Utc>,
    ) -> Result<LicenseKey, ServerError> {
        let now = Utc::now();
        
        // First, deactivate any existing licenses for this user
        sqlx::query(
            r#"
            UPDATE license_keys
            SET is_active = false, updated_at = $1
            WHERE user_id = $2 AND is_active = true
            "#
        )
        .bind(now)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| {
            error!("Database error deactivating existing licenses: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        // Create the new license
        let license = sqlx::query_as::<_, LicenseKey>(
            r#"
            INSERT INTO license_keys (
                id, user_id, license_key, is_active, issued_at,
                expires_at, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(license_key)
        .bind(true)  // is_active
        .bind(now)   // issued_at
        .bind(expires_at)
        .bind(now)
        .bind(now)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Database error creating license: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Created new license key for user: {}", user_id);
        Ok(license)
    }
    
    /// Verify a license key
    pub async fn verify(
        pool: &PgPool, 
        license_key: &str
    ) -> Result<Option<LicenseKey>, ServerError> {
        let now = Utc::now();
        
        // Verify and update the last_verified_at timestamp
        let license = sqlx::query_as::<_, LicenseKey>(
            r#"
            UPDATE license_keys
            SET last_verified_at = $1, updated_at = $1
            WHERE license_key = $2 AND is_active = true AND expires_at > $1
            RETURNING *
            "#
        )
        .bind(now)
        .bind(license_key)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            error!("Database error verifying license: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        if let Some(ref license) = license {
            info!("Verified license key for user: {}", license.user_id);
        }
        
        Ok(license)
    }
    
    /// Deactivate a license key
    pub async fn deactivate(
        pool: &PgPool,
        license_id: Uuid,
    ) -> Result<LicenseKey, ServerError> {
        let now = Utc::now();
        
        let license = sqlx::query_as::<_, LicenseKey>(
            r#"
            UPDATE license_keys
            SET is_active = false, updated_at = $1
            WHERE id = $2
            RETURNING *
            "#
        )
        .bind(now)
        .bind(license_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            error!("Database error deactivating license: {}", e);
            ServerError::Database(e.to_string())
        })?;
        
        info!("Deactivated license key for user: {}", license.user_id);
        Ok(license)
    }
}