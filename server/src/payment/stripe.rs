use axum::{
    extract::{Json, State, Request},
    response::IntoResponse,
    http::StatusCode,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use log::{info, error, warn};

use crate::auth::jwt::{AuthenticatedUser, create_license_token};
use crate::AppState;
use crate::db::models::{SubscriptionPlan, SubscriptionStatus};
use crate::db::operations::{UserOps, SubscriptionOps, LicenseOps};
use crate::errors::ServerError;

/// Client-side model for subscription plans
#[derive(Debug, Serialize)]
pub struct PlanResponse {
    id: String,
    name: String,
    description: String,
    #[serde(rename = "priceMonthly")]
    price_monthly: f64,
    #[serde(rename = "priceYearly")]
    price_yearly: f64,
    features: Vec<String>,
}

impl From<&SubscriptionPlan> for PlanResponse {
    fn from(plan: &SubscriptionPlan) -> Self {
        PlanResponse {
            id: plan.id.clone(),
            name: plan.name.clone(),
            description: plan.description.clone(),
            price_monthly: (plan.price_monthly as f64) / 100.0,
            price_yearly: (plan.price_yearly as f64) / 100.0,
            features: plan.features.clone(),
        }
    }
}

/// Response for list of available plans
#[derive(Debug, Serialize)]
pub struct PlansResponse {
    pub plans: Vec<PlanResponse>,
}

/// Request for creating a checkout session
#[derive(Debug, Deserialize)]
pub struct CheckoutRequest {
    pub plan_id: String,
    pub billing_cycle: BillingCycle,
}

/// Billing cycle options
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BillingCycle {
    Monthly,
    Yearly,
}

/// Response with checkout session details
#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    checkout_url: String,
    session_id: String,
}

/// Create a checkout session for subscription (simplified implementation)
pub async fn create_checkout(
    auth_user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<CheckoutRequest>,
) -> Result<impl IntoResponse, ServerError> {
    // Generate fake data for demo
    let session_id = format!("cs_{}", Uuid::new_v4().simple());
    let checkout_url = format!("https://checkout.stripe.com/c/pay/{}", session_id);
    
    info!("Created checkout session for user {}", auth_user.user_id);
    
    // Return the checkout URL
    let response = CheckoutResponse {
        checkout_url,
        session_id,
    };
    
    Ok((StatusCode::OK, Json(response)))
}

/// Handle incoming webhook events (simplified implementation)
pub async fn handle_webhook(
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // Just return OK for demo purposes
    Ok(StatusCode::OK)
}

/// Get current license information (simplified implementation)
pub async fn get_license(
    auth_user: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    #[derive(Serialize)]
    struct LicenseResponse {
        license_key: String,
        expires_at: DateTime<Utc>,
        subscription_active: bool,
    }
    
    // Generate fake data for demo
    let now = Utc::now();
    let expires_at = now + Duration::days(365);
    let license_key = format!("license_{}", Uuid::new_v4().simple());
    
    let response = LicenseResponse {
        license_key,
        expires_at,
        subscription_active: true,
    };
    
    Ok((StatusCode::OK, Json(response)))
}

/// Check if a subscription status is considered active
pub fn is_active_status(status: &SubscriptionStatus) -> bool {
    match status {
        SubscriptionStatus::Active | SubscriptionStatus::Trialing => true,
        _ => false,
    }
}

/// Map a string status to our enum
pub fn map_stripe_status(status: &str) -> SubscriptionStatus {
    match status {
        "active" => SubscriptionStatus::Active,
        "trialing" => SubscriptionStatus::Trialing,
        "past_due" => SubscriptionStatus::PastDue,
        "canceled" => SubscriptionStatus::Canceled,
        "unpaid" => SubscriptionStatus::Unpaid,
        "incomplete" => SubscriptionStatus::Incomplete,
        "incomplete_expired" => SubscriptionStatus::IncompleteExpired,
        _ => {
            warn!("Unknown subscription status: {}", status);
            SubscriptionStatus::Canceled
        }
    }
}