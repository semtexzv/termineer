//! Authentication API
//!
//! API endpoints for authentication status.

use crate::auth::session::SessionData;
use axum::{extract::Extension, Json};
use serde_json::{json, Value};

/// Get current user authentication status
pub async fn get_status(extension: Extension<SessionData>) -> Json<Value> {
    let response = if let Some(user) = &extension.user {
        json!({
            "authenticated": true,
            "user": {
                "id": user.id,
                "email": user.email,
                "name": user.name,
                "picture": user.picture,
                "subscription": user.subscription
            }
        })
    } else {
        json!({
            "authenticated": false,
            "user": null
        })
    };

    Json(response)
}
