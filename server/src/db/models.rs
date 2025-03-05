use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// User model representing a registered user
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub auth_provider: String,
    pub auth_provider_id: Option<String>,
    pub is_active: bool,
    pub has_subscription: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Subscription model representing a user's subscription
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Subscription {
    pub id: Uuid,
    pub user_id: Uuid,
    pub stripe_customer_id: String,
    pub stripe_subscription_id: String,
    pub plan_id: String,
    pub status: SubscriptionStatus,
    pub current_period_start: DateTime<Utc>,
    pub current_period_end: DateTime<Utc>,
    pub cancel_at_period_end: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// License key model for tracking issued licenses
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct LicenseKey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub license_key: String,
    pub is_active: bool,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Usage statistics for analytics
#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct UsageStat {
    pub id: Uuid,
    pub user_id: Uuid,
    pub license_id: Uuid,
    pub event_type: String,
    pub event_data: serde_json::Value,
    pub client_version: Option<String>,
    pub client_platform: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Subscription status enum
#[derive(Debug, Serialize, Deserialize, sqlx::Type, PartialEq, Clone)]
#[sqlx(type_name = "subscription_status", rename_all = "lowercase")]
pub enum SubscriptionStatus {
    Active,
    Trialing,
    PastDue,
    Canceled,
    Unpaid,
    Incomplete,
    IncompleteExpired,
}

/// Subscription plan details
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubscriptionPlan {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price_monthly: i64,  // Price in cents
    pub price_yearly: i64,   // Price in cents
    pub features: Vec<String>,
    pub is_active: bool,
}

impl SubscriptionPlan {
    /// Returns all available subscription plans
    pub fn all() -> Vec<Self> {
        vec![
            SubscriptionPlan {
                id: "basic".to_string(),
                name: "Basic Plan".to_string(),
                description: "Essential features for individual developers".to_string(),
                price_monthly: 1499, // $14.99
                price_yearly: 14990, // $149.90 (2 months free)
                features: vec![
                    "Full access to AutoSWE CLI".to_string(),
                    "Basic model support".to_string(),
                    "Standard tools".to_string(),
                    "Community support".to_string(),
                ],
                is_active: true,
            },
            SubscriptionPlan {
                id: "professional".to_string(),
                name: "Professional Plan".to_string(),
                description: "Advanced features for professional developers".to_string(),
                price_monthly: 2999, // $29.99
                price_yearly: 29990, // $299.90 (2 months free)
                features: vec![
                    "Everything in Basic".to_string(),
                    "Premium model support".to_string(),
                    "Advanced tools".to_string(),
                    "Email support".to_string(),
                    "Higher rate limits".to_string(),
                ],
                is_active: true,
            },
            SubscriptionPlan {
                id: "enterprise".to_string(),
                name: "Enterprise Plan".to_string(),
                description: "Comprehensive solution for teams and organizations".to_string(),
                price_monthly: 9999, // $99.99
                price_yearly: 99990, // $999.90 (2 months free)
                features: vec![
                    "Everything in Professional".to_string(),
                    "Team management".to_string(),
                    "Custom model fine-tuning".to_string(),
                    "Priority support".to_string(),
                    "Custom integrations".to_string(),
                    "Usage analytics".to_string(),
                ],
                is_active: true,
            },
        ]
    }
    
    /// Find a plan by ID
    pub fn find_by_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|p| p.id == id)
    }
}