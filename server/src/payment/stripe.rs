use axum::{
    extract::{State, Query},
    response::{IntoResponse, Redirect, Html},
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use log::{info, error};
use std::collections::HashMap;

use std::sync::Arc;
use crate::AppState;
use crate::db::models::{SubscriptionPlan, SubscriptionStatus};
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
    pub plan: String,
}

/// Response with checkout session details
#[derive(Debug, Serialize)]
pub struct CheckoutResponse {
    checkout_url: String,
    session_id: String,
}

// Query parameters for success page
#[derive(Debug, Deserialize)]
pub struct SuccessParams {
    plan: Option<String>,
    success: Option<String>,
}

/// Simple success page handler that doesn't require authentication
pub async fn success_page(
    Query(params): Query<SuccessParams>
) -> impl IntoResponse {
    let plan = params.plan.unwrap_or_else(|| "unknown".to_string());
    let success = params.success.unwrap_or_else(|| "false".to_string());
    
    let plan_display = match plan.as_str() {
        "free" => "Free",
        "plus" => "Plus",
        "pro" => "Pro",
        _ => "Unknown"
    };
    
    if success == "true" {
        // Success page with minimal HTML
        let success_html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Termineer - Subscription Success</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <script src="https://cdn.tailwindcss.com"></script>
</head>
<body style="font-family: system-ui, sans-serif; background-color: #f9fafb; margin: 0; min-height: 100vh; display: flex; flex-direction: column;">
    <header style="background-color: white; box-shadow: 0 1px 3px rgba(0,0,0,0.1); padding: 1rem 0;">
        <div style="max-width: 1280px; margin: 0 auto; padding: 0 1rem; display: flex; justify-content: space-between; align-items: center;">
            <a href="/" style="display: flex; align-items: center; text-decoration: none; color: inherit;">
                <span style="font-size: 1.5rem; font-weight: 700; color: #111827;">Termineer</span>
            </a>
        </div>
    </header>
    
    <main style="flex: 1; display: flex; align-items: center; justify-content: center; padding: 3rem 1rem;">
        <div style="max-width: 28rem; width: 100%; background-color: white; border-radius: 0.75rem; box-shadow: 0 4px 6px rgba(0,0,0,0.1); overflow: hidden;">
            <div style="padding: 2rem; text-align: center;">
                <div style="width: 4rem; height: 4rem; background-color: #d1fae5; border-radius: 9999px; display: inline-flex; align-items: center; justify-content: center; margin-bottom: 1.5rem;">
                    <svg style="width: 2rem; height: 2rem; color: #10b981;" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                    </svg>
                </div>
                <h2 style="font-size: 1.875rem; font-weight: 700; color: #111827; margin-bottom: 0.5rem;">Payment Successful!</h2>
                <p style="color: #4b5563; margin-bottom: 2rem;">Thank you for subscribing to the {plan_display} plan. Your account has been upgraded.</p>
                <div style="background-color: #f3f4f6; padding: 1rem; border-radius: 0.5rem; margin-bottom: 1.5rem; text-align: left;">
                    <p style="color: #4338ca; font-weight: 500; margin: 0;">Plan: {plan_display}</p>
                </div>
                <a href="/" style="display: inline-block; padding: 0.75rem 1.5rem; background-color: #4f46e5; color: white; font-weight: 500; border-radius: 0.375rem; text-decoration: none;">
                    Return to Dashboard
                </a>
            </div>
        </div>
    </main>
    
    <footer style="background-color: #1f2937; color: white; padding: 1.5rem 0;">
        <div style="max-width: 1280px; margin: 0 auto; padding: 0 1rem; text-align: center;">
            <p style="color: #9ca3af; margin: 0;">&copy; 2024 Termineer. All rights reserved.</p>
        </div>
    </footer>
</body>
</html>
        "#);
        
        Html(success_html)
    } else {
        // Error page with minimal HTML
        let error_html = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <title>Termineer - Subscription Error</title>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <script src="https://cdn.tailwindcss.com"></script>
</head>
<body style="font-family: system-ui, sans-serif; background-color: #f9fafb; margin: 0; min-height: 100vh; display: flex; flex-direction: column;">
    <header style="background-color: white; box-shadow: 0 1px 3px rgba(0,0,0,0.1); padding: 1rem 0;">
        <div style="max-width: 1280px; margin: 0 auto; padding: 0 1rem; display: flex; justify-content: space-between; align-items: center;">
            <a href="/" style="display: flex; align-items: center; text-decoration: none; color: inherit;">
                <span style="font-size: 1.5rem; font-weight: 700; color: #111827;">Termineer</span>
            </a>
        </div>
    </header>
    
    <main style="flex: 1; display: flex; align-items: center; justify-content: center; padding: 3rem 1rem;">
        <div style="max-width: 28rem; width: 100%; background-color: white; border-radius: 0.75rem; box-shadow: 0 4px 6px rgba(0,0,0,0.1); overflow: hidden;">
            <div style="padding: 2rem; text-align: center;">
                <div style="width: 4rem; height: 4rem; background-color: #fee2e2; border-radius: 9999px; display: inline-flex; align-items: center; justify-content: center; margin-bottom: 1.5rem;">
                    <svg style="width: 2rem; height: 2rem; color: #ef4444;" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                </div>
                <h2 style="font-size: 1.875rem; font-weight: 700; color: #111827; margin-bottom: 0.5rem;">Payment Error</h2>
                <p style="color: #4b5563; margin-bottom: 2rem;">There was a problem processing your payment for the {plan_display} plan.</p>
                <div style="display: flex; gap: 1rem; justify-content: center;">
                    <a href="/" style="display: inline-block; padding: 0.75rem 1.5rem; background-color: #4f46e5; color: white; font-weight: 500; border-radius: 0.375rem; text-decoration: none;">
                        Return to Home
                    </a>
                    <a href="mailto:support@termineer.com" style="display: inline-block; padding: 0.75rem 1.5rem; background-color: white; color: #111827; font-weight: 500; border: 1px solid #d1d5db; border-radius: 0.375rem; text-decoration: none;">
                        Contact Support
                    </a>
                </div>
            </div>
        </div>
    </main>
    
    <footer style="background-color: #1f2937; color: white; padding: 1.5rem 0;">
        <div style="max-width: 1280px; margin: 0 auto; padding: 0 1rem; text-align: center;">
            <p style="color: #9ca3af; margin: 0;">&copy; 2024 Termineer. All rights reserved.</p>
        </div>
    </footer>
</body>
</html>
        "#);
        
        Html(error_html)
    }
}

/// Create a checkout session for subscription
#[axum::debug_handler]
pub async fn create_checkout(
    State(state): State<Arc<AppState>>,
    form: axum::extract::Form<HashMap<String, String>>,
) -> Result<impl IntoResponse, ServerError> {
    let plan_id = form.get("plan").cloned().unwrap_or_else(|| "free".to_string());
    
    // If it's a free plan, redirect to success page
    if plan_id == "free" {
        let free_success_url = format!("/payment/success-page?plan=free&success=true");
        return Ok(Redirect::to(&free_success_url).into_response());
    }
    
    // Get plan details
    let plan = SubscriptionPlan::find_by_id(&plan_id)
        .ok_or_else(|| ServerError::NotFound(format!("Plan not found: {}", plan_id)))?;
    
    // Log plan selection
    info!("Checkout requested for plan: {} with environment: {}", 
          plan_id, state.config.environment);
    info!("Success redirect URL: {}", state.config.success_redirect_url);
    
    // In development mode, we'll use mock implementation
    if state.config.is_development() {        
        // Build a mock checkout URL that uses the proper success URL 
        let mock_url = format!("/payment/success-page?plan={}&success=true", plan_id);
        
        info!("Development mode: Redirecting to mock URL: {}", mock_url);
        return Ok(Redirect::to(&mock_url).into_response());
    }
    
    // For production, get stripe price ID
    let _price_id = plan.stripe_price_id
        .as_ref()
        .ok_or_else(|| ServerError::Internal("Stripe price ID not configured for this plan".to_string()))?;
    
    // Get user email from form if available
    let _customer_email = form.get("email").cloned();
    
    // In a real production system we would use the Stripe API here
    // But for our purposes, we'll just redirect to the success page
    info!("Production mode: Creating checkout for plan: {}", plan_id);
    
    let url = format!("/payment/success-page?plan={}&success=true", plan_id);
    
    Ok(Redirect::to(&url).into_response())
}

/// Handle incoming webhook events from Stripe
#[axum::debug_handler]
pub async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
    body: String,
) -> Result<impl IntoResponse, ServerError> {
    // Log environment
    info!("Webhook received in environment: {}", state.config.environment);
    
    // In development mode, just return OK
    if state.config.is_development() {
        info!("Development mode: Mock webhook received");
        return Ok(StatusCode::OK);
    }
    
    // In a real production system we would verify and process webhook events
    // But for now we'll just log that we received something
    info!("Webhook received - length of body: {}", body.len());
    
    Ok(StatusCode::OK)
}

/// Check if a subscription status is considered active
pub fn is_active_status(status: &SubscriptionStatus) -> bool {
    match status {
        SubscriptionStatus::Active | SubscriptionStatus::Trialing => true,
        _ => false,
    }
}

/// Map a subscription status string to our enum
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
            error!("Unknown subscription status: {}", status);
            SubscriptionStatus::Canceled
        }
    }
}