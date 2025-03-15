//! Google OAuth authentication provider
//!
//! Implements authentication using Google's OAuth2 protocol.

use crate::config::Config;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use reqwest::Client as HttpClient;
use serde::Deserialize;
use std::error::Error;

type BasicClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

/// Create an OAuth client for Google authentication
pub fn create_oauth_client(config: &Config) -> Result<BasicClient, Box<dyn Error>> {
    let google_client_id = config
        .oauth
        .google_client_id
        .clone()
        .ok_or("Google OAuth client ID not configured")?;

    let google_client_secret = config
        .oauth
        .google_client_secret
        .clone()
        .ok_or("Google OAuth client secret not configured")?;

    let redirect_uri = config.oauth.google_redirect_uri.clone().unwrap_or_else(|| {
        format!(
            "http://{}:{}/auth/google/callback",
            config.host, config.port
        )
    });

    let client = oauth2::basic::BasicClient::new(ClientId::new(google_client_id))
        .set_client_secret(ClientSecret::new(google_client_secret))
        .set_auth_uri(AuthUrl::new(
            "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
        )?)
        .set_token_uri(TokenUrl::new(
            "https://oauth2.googleapis.com/token".to_string(),
        )?)
        .set_redirect_uri(RedirectUrl::new(redirect_uri)?);

    Ok(client)
}

/// Generate an authorization URL for Google authentication
pub fn generate_auth_url(client: &BasicClient) -> (String, CsrfToken, PkceCodeVerifier) {
    // Create a PKCE code verifier and challenge
    let (pkce_code_challenge, pkce_code_verifier) = PkceCodeChallenge::new_random_sha256();

    // Generate the authorization URL
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_code_challenge)
        .url();

    (auth_url.to_string(), csrf_token, pkce_code_verifier)
}

/// User information from Google
#[derive(Debug, Deserialize)]
pub struct GoogleUserInfo {
    pub id: String,
    pub email: String,
    pub verified_email: bool,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub picture: Option<String>,
    pub locale: Option<String>,
}

/// Exchange an authorization code for tokens and retrieve user info
pub async fn exchange_code_and_get_user_info(
    client: &BasicClient,
    code: AuthorizationCode,
    pkce_verifier: PkceCodeVerifier,
) -> Result<GoogleUserInfo, Box<dyn Error>> {
    // Exchange the code for an access token
    let token_response = client
        .exchange_code(code)
        .set_pkce_verifier(pkce_verifier)
        .request_async(&reqwest::Client::new())
        .await?;

    // Get the access token
    let access_token = token_response.access_token().secret();

    // Use the access token to get user info
    let client = HttpClient::new();
    let user_info = client
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .bearer_auth(access_token)
        .send()
        .await?
        .json::<GoogleUserInfo>()
        .await?;

    Ok(user_info)
}
