use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const ACCESS_COOKIE_NAME: &str = "bb_access";
pub const REFRESH_COOKIE_NAME: &str = "bb_refresh";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub access_expires_in: usize,
    pub refresh_expires_in: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub token_type: TokenType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum TokenType {
    Access,
    Refresh,
}

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
