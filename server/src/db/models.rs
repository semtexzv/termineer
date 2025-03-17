use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row};
use uuid::Uuid;

/// User model representing a registered user
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
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
    pub google_access_token: Option<String>,
    pub google_refresh_token: Option<String>,
    pub token_expires_at: Option<DateTime<Utc>>,
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
    pub price_monthly: i64, // Price in cents
    pub price_yearly: i64,  // Price in cents
    pub features: Vec<String>,
    pub is_active: bool,
    pub stripe_price_id: Option<String>, // Stripe price ID for the plan
}

impl SubscriptionPlan {
    /// Returns all available subscription plans
    pub fn all() -> Vec<Self> {
        vec![
            SubscriptionPlan {
                id: "free".to_string(),
                name: "Free".to_string(),
                description: "Basic features for personal use".to_string(),
                price_monthly: 0, // $0
                price_yearly: 0,  // $0
                features: vec![
                    "Limited API requests".to_string(),
                    "Basic AI models".to_string(),
                    "Console access".to_string(),
                ],
                is_active: true,
                stripe_price_id: None, // Free plan doesn't have a Stripe price
            },
            SubscriptionPlan {
                id: "plus".to_string(),
                name: "Plus".to_string(),
                description: "Advanced features for professionals".to_string(),
                price_monthly: 1200, // $12.00
                price_yearly: 12000, // $120.00 (2 months free)
                features: vec![
                    "Unlimited API requests".to_string(),
                    "Access to Claude and Gemini models".to_string(),
                    "Full tool support".to_string(),
                ],
                is_active: true,
                stripe_price_id: Some(
                    std::env::var("STRIPE_PRICE_ID_PLUS_MONTHLY")
                        .unwrap_or_else(|_| "price_plus_monthly".to_string()),
                ),
            },
            SubscriptionPlan {
                id: "pro".to_string(),
                name: "Pro".to_string(),
                description: "Premium features for teams".to_string(),
                price_monthly: 2900, // $29.00
                price_yearly: 29000, // $290.00 (2 months free)
                features: vec![
                    "Everything in Plus".to_string(),
                    "Priority access to newest models".to_string(),
                    "Team collaboration features".to_string(),
                ],
                is_active: true,
                stripe_price_id: Some(
                    std::env::var("STRIPE_PRICE_ID_PRO_MONTHLY")
                        .unwrap_or_else(|_| "price_pro_monthly".to_string()),
                ),
            },
        ]
    }

    /// Find a plan by ID
    pub fn find_by_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|p| p.id == id)
    }
}
