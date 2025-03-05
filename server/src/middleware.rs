use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{Response, IntoResponse},
    http::{StatusCode, Method, HeaderName, HeaderValue},
    Json,
};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

use crate::AppState;
use crate::auth::jwt::AuthenticatedUser;

/// Configure CORS for the application
pub fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
        .allow_origin(Any) // In production, limit to specific origins
}

/// Middleware to check if a user has an active subscription
pub async fn require_subscription(
    auth_user: AuthenticatedUser,
    request: Request,
    next: Next,
) -> Response {
    if !auth_user.has_subscription {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "authorization_error",
                "message": "Active subscription required",
                "status_code": 403
            }))
        ).into_response();
    }

    // Pass the authenticated user to the handler
    let mut request = request;
    request.extensions_mut().insert(auth_user);
    next.run(request).await
}

/// Add rate limiting headers to responses
pub async fn add_rate_limit_headers<B>(response: Response<B>) -> Response<B> {
    let mut response = response;
    
    // Add rate limiting headers
    let headers = response.headers_mut();
    headers.insert(
        HeaderName::from_static("x-rate-limit-limit"),
        HeaderValue::from_static("100"),
    );
    headers.insert(
        HeaderName::from_static("x-rate-limit-remaining"),
        HeaderValue::from_static("99"),
    );
    headers.insert(
        HeaderName::from_static("x-rate-limit-reset"),
        HeaderValue::from_static("1614556800"),
    );
    
    response
}

/// Track usage for analytics
pub async fn track_usage(
    State(state): State<AppState>,
    auth_user: Option<AuthenticatedUser>,
    request: Request,
    next: Next,
) -> Response {
    // Start timing
    let start = std::time::Instant::now();
    
    // Store the URI before we consume the request
    let uri = request.uri().clone();
    
    // Process the request
    let response = next.run(request).await;
    
    // Calculate duration
    let duration = start.elapsed();
    
    // Log request details
    if let Some(user) = auth_user {
        // In a real implementation, we would record this to the database
        // For demo purposes, just log it
        log::info!(
            "User {}: {} {} - {} ms",
            user.user_id,
            response.status(),
            uri,
            duration.as_millis()
        );
    } else {
        log::info!(
            "Anonymous: {} {} - {} ms",
            response.status(),
            uri,
            duration.as_millis()
        );
    }
    
    response
}