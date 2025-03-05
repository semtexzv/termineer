use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use log::{info, error};

use crate::auth::jwt::AuthenticatedUser;
use std::sync::Arc;
use crate::AppState;
use crate::db::models::SubscriptionPlan;
use crate::db::operations::{SubscriptionOps, LicenseOps};
use crate::payment::stripe;
use crate::errors::ServerError;

/// List all available subscription plans
pub async fn list_plans() -> Result<impl IntoResponse, ServerError> {
    // Use the stripe module's implementation
    let plans = SubscriptionPlan::all();
    let response = stripe::PlansResponse {
        plans: plans.iter().map(stripe::PlanResponse::from).collect(),
    };
    
    Ok((StatusCode::OK, Json(response)))
}

/// Subscription details for the response
#[derive(Debug, Serialize)]
struct SubscriptionResponse {
    id: Uuid,
    plan_id: String,
    plan_name: String,
    status: String,
    current_period_end: chrono::DateTime<chrono::Utc>,
    cancel_at_period_end: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Get the current user's subscription
#[axum::debug_handler]
pub async fn get_subscription(
    auth_user: AuthenticatedUser,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ServerError> {
    // Check if user has a subscription
    if !auth_user.has_subscription {
        return Ok((StatusCode::OK, Json(serde_json::json!({
            "has_subscription": false
        }))));
    }
    
    // Get subscription details
    let subscription = SubscriptionOps::find_by_user_id(&state.db_pool, auth_user.user_id).await?
        .ok_or_else(|| ServerError::NotFound("Subscription not found".to_string()))?;
    
    // Get plan details
    let plan = SubscriptionPlan::find_by_id(&subscription.plan_id)
        .unwrap_or_else(|| SubscriptionPlan::all()[0].clone());
    
    let response = SubscriptionResponse {
        id: subscription.id,
        plan_id: subscription.plan_id,
        plan_name: plan.name,
        status: format!("{:?}", subscription.status).to_lowercase(),
        current_period_end: subscription.current_period_end,
        cancel_at_period_end: subscription.cancel_at_period_end,
        created_at: subscription.created_at,
    };
    
    Ok((StatusCode::OK, Json(serde_json::json!({
        "has_subscription": true,
        "subscription": response
    }))))
}

/// Response for subscription management portal URL
#[derive(Debug, Serialize)]
struct PortalResponse {
    url: String,
}

/// Create a Stripe customer portal session to manage subscription
pub async fn subscription_portal(
    auth_user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ServerError> {
    // Check if user has a subscription
    if !auth_user.has_subscription {
        return Err(ServerError::BadRequest("No active subscription found".to_string()));
    }
    
    // Get subscription details
    let subscription = SubscriptionOps::find_by_user_id(&state.db_pool, auth_user.user_id).await?
        .ok_or_else(|| ServerError::NotFound("Subscription not found".to_string()))?;
    
    // In a real implementation, we would connect to Stripe 
    // For demo purposes, we'll generate a mock URL to the Stripe dashboard
    let dashboard_url = format!(
        "https://dashboard.stripe.com/customers/{}", 
        subscription.stripe_customer_id
    );
    
    Ok((StatusCode::OK, Json(PortalResponse {
        url: dashboard_url,
    })))
}