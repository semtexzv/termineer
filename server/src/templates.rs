use askama::Template;

// User model for auth status
#[derive(Debug, Clone, Default)]
pub struct User {
    pub email: String,
    pub subscription: Option<String>,
}

// Template for the index page
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate;

// Template for the auth button component
#[derive(Template)]
#[template(path = "components/auth_button.html")]
pub struct AuthButtonTemplate {
    pub user: Option<User>,
}

// Template for the checkout form
#[derive(Template)]
#[template(path = "checkout.html")]
pub struct CheckoutTemplate {
    pub plan_name: String,
    pub checkout_url: String,
    pub email: String,
}