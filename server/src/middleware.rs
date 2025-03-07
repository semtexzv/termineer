use axum::{
    extract::{Request, Extension, State},
    middleware::Next,
    response::Response,
};
use tower_sessions::Session;
use crate::templates::User;
use crate::auth::jwt;
use crate::AppState;
use log::{info, debug, warn};
use std::sync::Arc;

// Middleware to attach user info to request if logged in
// Now accepts AppState as a parameter to work with from_fn_with_state
pub async fn attach_user(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    // Log the request path for debugging
    let path = request.uri().path().to_string();
    debug!("Running attach_user middleware for path: {}", path);
    
    // Add AppState to request extensions to make it available for authentication
    // We clone the Arc to avoid ownership issues
    println!("Middleware: Adding AppState to request extensions for path: {}", path);
    
    // First, check if AppState is already in extensions to avoid duplicates
    if request.extensions().get::<Arc<AppState>>().is_some() {
        debug!("AppState already exists in extensions");
    } else {
        // Add AppState to extensions
        request.extensions_mut().insert(state.clone());
        debug!("AppState inserted into request extensions");
        
        // Also log the JWT secret length for debugging (without revealing the secret)
        debug!("JWT secret length: {}", state.config.jwt_secret.len());
    }
    
    // Get session from request extensions
    let user = if let Some(session) = request.extensions().get::<Session>() {
        // Check for user info in session
        match session.get::<String>("user_email").await {
            Ok(Some(email)) => {
                info!("Found user email in session: {}", email);
                
                // Get additional user information
                let name = session.get::<String>("user_name").await.unwrap_or(None);
                let subscription = session.get::<String>("subscription_type").await.unwrap_or(None);
                let logged_in = session.get::<bool>("logged_in").await.unwrap_or(Some(false)).unwrap_or(false);
                
                // Make sure we add auth_token to the request extension as well if available
                if let Ok(Some(token)) = session.get::<String>("auth_token").await {
                    debug!("Found auth_token in session, adding to request extensions");
                    request.extensions_mut().insert(token);
                }
                
                if logged_in {
                    info!("User is logged in: {}", email);
                    
                    Some(User {
                        email,
                        name,
                        subscription,
                    })
                } else {
                    debug!("User email found but logged_in flag is false");
                    None
                }
            },
            Ok(None) => {
                debug!("No user email found in session");
                None
            },
            Err(e) => {
                warn!("Error retrieving user email from session: {}", e);
                None
            }
        }
    } else {
        debug!("No session found in request extensions for path: {}", path);
        None
    };
    
    // Add user to request extensions
    if user.is_some() {
        debug!("Adding user to request extensions");
        request.extensions_mut().insert(user);
    } else {
        debug!("No user to add to request extensions");
    }
    
    // Continue to the handler
    debug!("Completing middleware, continuing to handler");
    next.run(request).await
}