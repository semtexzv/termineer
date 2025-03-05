use axum::{
    extract::{Request, Extension},
    middleware::Next,
    response::Response,
};
use tower_sessions::Session;
use crate::templates::User;

// Middleware to attach user info to request if logged in
pub async fn attach_user(
    session: Session,
    mut request: Request,
    next: Next,
) -> Response {
    let user = if let Ok(Some(email)) = session.get::<String>("user_email").await {
        // Get subscription info if available
        let subscription = session.get::<String>("subscription_type").await.unwrap_or(None);
        
        Some(User {
            email,
            subscription,
        })
    } else {
        None
    };
    
    // Add user to request extensions
    request.extensions_mut().insert(user);
    
    // Continue to the handler
    next.run(request).await
}