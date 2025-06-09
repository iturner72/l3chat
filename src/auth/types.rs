use leptos::prelude::*;
use std::fmt;

pub const AUTH_COOKIE_NAME: &str = "auth_token";

#[derive(Debug)]
pub enum AuthError {
    TokenCreation(String),
    TokenVerification(String),
    TokenExpired,
    TokenRefreshFailed(String),
    InvalidCredentials,
    MissingEnvironmentVar(String),
    CookieError(String),
    DatabaseError(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::TokenCreation(e) => write!(f, "Failed to create token: {}", e),
            AuthError::TokenVerification(e) => write!(f, "Failed to verify token: {}", e),
            AuthError::TokenExpired => write!(f, "Token has expired"),
            AuthError::TokenRefreshFailed(e) => write!(f, "Failed to refresh token: {}", e),
            AuthError::InvalidCredentials => write!(f, "Invalid username or password"),
            AuthError::MissingEnvironmentVar(var) => {
                write!(f, "Missing environment variable: {}", var)
            }
            AuthError::CookieError(e) => write!(f, "Cookie error: {}", e),
            AuthError::DatabaseError(e) => write!(f, "Database error: {}", e),
        }
    }
}

pub fn to_server_error(e: AuthError) -> ServerFnError {
    ServerFnError::ServerError(e.to_string())
}
