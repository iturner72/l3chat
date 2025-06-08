#[cfg(feature = "ssr")]
pub mod jwt {
    use super::super::types::{AuthError, AuthResponse, TokenClaims, TokenType};
    use super::super::types::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME};
    use super::super::secure::verify_password;
    use axum_extra::extract::cookie::{Cookie, SameSite};
    use cookie::time;
    use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
    use std::time::{SystemTime, UNIX_EPOCH};

    const ACCESS_TOKEN_DURATION: usize = 15 * 60;
    const REFRESH_TOKEN_DURATION: usize = 7 * 24 * 60 * 60;

    pub async fn authenticate_admin(username: &str, password: &str) -> Result<bool, AuthError> {
        let admin_user = std::env::var("ADMIN_USERNAME")
            .map_err(|_| AuthError::MissingEnvironmentVar("ADMIN_USERNAME".to_string()))?;
            
        let stored_hash = std::env::var("ADMIN_PASSWORD_HASH")
            .map_err(|_| AuthError::MissingEnvironmentVar("ADMIN_PASSWORD_HASH".to_string()))?;
    
        if username != admin_user {
            return Ok(false);
        }
    
        match verify_password(password, &stored_hash) {
            Ok(valid) => {
                log::debug!("Password verification result: {valid}");
                Ok(valid)
            },
            Err(e) => {
                log::error!("Password verification error: {e}");
                Err(AuthError::TokenCreation(e))
            }
        }
    }

    pub fn generate_tokens(username: String) -> Result<AuthResponse, AuthError> {
        let jwt_secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let access_claims = TokenClaims {
            sub: username.clone(),
            exp: now + ACCESS_TOKEN_DURATION,
            iat: now,
            token_type: TokenType::Access,
        };

        let access_token = encode(
            &Header::default(),
            &access_claims,
            &EncodingKey::from_secret(jwt_secret.as_bytes())
        ).map_err(|e| AuthError::TokenCreation(e.to_string()))?;

        let refresh_claims = TokenClaims {
            sub: username,
            exp: now + REFRESH_TOKEN_DURATION,
            iat: now,
            token_type: TokenType::Refresh,
        };

        let refresh_token = encode(
            &Header::default(),
            &refresh_claims,
            &EncodingKey::from_secret(jwt_secret.as_bytes())
        ).map_err(|e| AuthError::TokenCreation(e.to_string()))?;

        Ok(AuthResponse {
            access_token,
            refresh_token,
            access_expires_in: ACCESS_TOKEN_DURATION,
            refresh_expires_in: REFRESH_TOKEN_DURATION,
        })
    }

    pub fn create_cookie_with_attributes(
        name: &'static str, 
        value: String,
        expires: time::OffsetDateTime,
    ) -> Cookie<'static> {
        Cookie::build((name, value))
            .path("/")
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Strict)
            .expires(expires)
            .build()
    }

    pub fn create_auth_cookies(auth_response: &AuthResponse) -> Vec<Cookie<'static>> {
        let now = time::OffsetDateTime::now_utc();
        vec![
            create_cookie_with_attributes(
                ACCESS_COOKIE_NAME,
                auth_response.access_token.clone(),
                now + time::Duration::minutes(15)
            ),
            create_cookie_with_attributes(
                REFRESH_COOKIE_NAME,
                auth_response.refresh_token.clone(),
                now + time::Duration::days(7)
            )
        ]
    }

    pub fn create_expired_cookies() -> Vec<Cookie<'static>> {
        let expired = time::OffsetDateTime::UNIX_EPOCH;
        vec![
            create_cookie_with_attributes(ACCESS_COOKIE_NAME, String::new(), expired),
            create_cookie_with_attributes(REFRESH_COOKIE_NAME, String::new(), expired)
        ]
    }

    pub fn verify_and_refresh_tokens(
        access_token: Option<&str>,
        refresh_token: Option<&str>,
    ) -> Result<Option<AuthResponse>, AuthError> {
        log::debug!("Token verification started");
        log::debug!("Access token present: {}", access_token.is_some());
        log::debug!("Refresh token present: {}", refresh_token.is_some());

        if refresh_token.is_none() {
            log::debug!("No refresh token, considering session invalid");
            return Err(AuthError::TokenExpired);
        }

        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| AuthError::MissingEnvironmentVar("JWT_SECRET".to_string()))?;

        let validation = Validation::default();

        if let Some(token) = access_token {
            log::debug!("Verifying access token");
            match decode::<TokenClaims>(
                token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
                &validation
            ) {
                Ok(token_data) => {
                    log::debug!("Access token decoded successfully");
                    if token_data.claims.token_type != TokenType::Access {
                        log::debug!("Invalid token type: expected Access");
                        return Err(AuthError::TokenVerification("Invalid token type".to_string()));
                    }
                    log::debug!("Access token is valid");
                    return Ok(None);
                },
                Err(e) => {
                    log::debug!("Access token verification failed: {e}");
                    if e.kind() != &jsonwebtoken::errors::ErrorKind::ExpiredSignature {
                        return Err(AuthError::TokenVerification(e.to_string()));
                    }
                    log::debug!("Access token expired, attempting refresh");
                }
            }
        }

        if let Some(token) = refresh_token {
            log::debug!("Attempting token refresh");
            match decode::<TokenClaims>(
                token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
                &validation
            ) {
                Ok(token_data) => {
                    log::debug!("Refresh token decoded successfully");
                    if token_data.claims.token_type != TokenType::Refresh {
                        log::debug!("Invalid token type: expected Refresh");
                        return Err(AuthError::TokenVerification("Invalid token type".to_string()));
                    }

                    log::debug!("Generating new tokens");
                    let new_tokens = generate_tokens(token_data.claims.sub)?;
                    log::debug!("New tokens generated successfully");
                    Ok(Some(new_tokens))
                },
                Err(e) => {
                    log::debug!("Refresh token verification failed: {e}");
                    Err(AuthError::TokenVerification(e.to_string()))
                }
            }
        } else {
            log::debug!("No refresh token available");
            Err(AuthError::TokenExpired)
        }

    }
}

#[cfg(feature = "ssr")]
pub mod middleware {
    use axum::{
        body::Body,
        http::{Request, HeaderValue, header},
        middleware::Next,
        response::{Response, IntoResponse},
        http::StatusCode,
    };
    use axum_extra::extract::cookie::CookieJar;
    use super::super::types::{ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME};
    use super::jwt;

    pub async fn require_auth(
        cookie_jar: CookieJar,
        request: Request<Body>,
        next: Next,
    ) -> Result<Response, StatusCode> {
        log::info!(
            "Auth middleware - Processing request to: {} {}",
            request.method(),
            request.uri()
        );

        let access_token = cookie_jar.get(ACCESS_COOKIE_NAME).map(|c| c.value().to_string());
        let refresh_token = cookie_jar.get(REFRESH_COOKIE_NAME).map(|c| c.value().to_string());

        log::debug!(
            "Auth middleware - Found tokens - Access: {}, Refresh: {}",
            access_token.is_some(),
            refresh_token.is_some()
        );

        match jwt::verify_and_refresh_tokens(
            access_token.as_deref(),
            refresh_token.as_deref(),
        ) {
            Ok(maybe_new_tokens) => {
                let mut response = next.run(request).await;

                if let Some(new_tokens) = maybe_new_tokens {
                    log::debug!("Auth middleware - Setting refreshed tokens in cookies");
                    let cookies = jwt::create_auth_cookies(&new_tokens);
                    for cookie in cookies {
                        log::debug!("Auth middleware - Setting cookie: {}", cookie.name());
                        if let Ok(cookie_value) = HeaderValue::from_str(&cookie.to_string()) {
                            response.headers_mut()
                                .append(header::SET_COOKIE, cookie_value);
                        }
                    }
                } else {
                    log::debug!("Auth middleware - Using existing valid tokens");
                }

                Ok(response)
            },
            Err(e) => {
                log::debug!("Auth middleware - Authentication failed: {e:?}");
                let mut response = StatusCode::UNAUTHORIZED.into_response();
                log::debug!("Auth middleware - Clearing invalid cookies");

                let expired_cookies = jwt::create_expired_cookies();
                for cookie in expired_cookies {
                    if let Ok(cookie_value) = HeaderValue::from_str(&cookie.to_string()) {
                        response.headers_mut()
                            .append(header::SET_COOKIE, cookie_value);
                    }
                }

                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Once;
    use once_cell::sync::Lazy;
    use tokio::sync::Mutex;
    use jsonwebtoken;
    use crate::auth::AuthError;
    use crate::auth::types::{TokenType, TokenClaims};

    // global mutex for environment variable operations
    static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
    static INIT: Once = Once::new();

    async fn initialize() {
        INIT.call_once(|| {
            // Initialize logging for tests
            let _ = env_logger::builder()
                .filter_level(log::LevelFilter::Debug)
                .is_test(true)
                .try_init();

            env::set_var("JWT_SECRET", "test_secret_for_testing_only");
            env::set_var("ADMIN_USERNAME", "test_admin");
            // Test hash for password "test_password"
            env::set_var("ADMIN_PASSWORD_HASH", "JGFyZ29uMmlkJHY9MTkkbT0xOTQ1Nix0PTIscD0xJDBOM2l6OGtESkpBTVZ1T0grMnlIWEEkY0RmbjhuaUp4bjJ6SE9kbFlGVUErT2VsZmV5enJXUG1McWtXODBFVHRnYw==");
            
            log::debug!("Test environment initialized");
        });
    }

    struct EnvVarGuard {
        vars: Vec<String>,
        previous_values: std::collections::HashMap<String, Option<String>>,
    }

    impl EnvVarGuard {
        fn new(vars: Vec<String>) -> Self {
            let mut previous_values = std::collections::HashMap::new();
            for var in &vars {
                previous_values.insert(var.clone(), env::var(var).ok());
                env::remove_var(var);
            }
            log::debug!("Environment variables temporarily cleared: {vars:?}");
            Self { vars, previous_values }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            for var in &self.vars {
                if let Some(Some(value)) = self.previous_values.get(var) {
                    env::set_var(var, value);
                } else {
                    env::remove_var(var);
                }
            }
            log::debug!("Environment variables restored");
        }
    }

    mod jwt_tests {
        use super::*;
        use std::time::{SystemTime,UNIX_EPOCH};
        use jsonwebtoken::{decode, DecodingKey, Validation};

        #[tokio::test]
        async fn test_generate_tokens() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;

            log::debug!("Testing token generation");
            let result = jwt::generate_tokens("test_user".to_string());
            assert!(result.is_ok(), "Token generation should succeed");

            let auth_response = result.unwrap();
            assert!(!auth_response.access_token.is_empty(), "Access token should not be empty");
            assert!(!auth_response.refresh_token.is_empty(), "Refresh token should not be empty");

            // Verify token types
            let jwt_secret = env::var("JWT_SECRET").unwrap();
            let access_claims = decode::<TokenClaims>(
                &auth_response.access_token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
                &Validation::default()
            ).unwrap().claims;

            let refresh_claims = decode::<TokenClaims>(
                &auth_response.refresh_token,
                &DecodingKey::from_secret(jwt_secret.as_bytes()),
                &Validation::default()
            ).unwrap().claims;

            assert_eq!(access_claims.token_type, TokenType::Access);
            assert_eq!(refresh_claims.token_type, TokenType::Refresh);
            
            log::debug!("Token generation test completed successfully");
        }

        #[tokio::test]
        async fn test_token_refresh_flow() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;
        
            log::debug!("Testing token refresh flow");
            
            // Generate initial tokens
            let initial_tokens = jwt::generate_tokens("test_user".to_string())
                .expect("Token generation should succeed");
        
            // Initially both tokens should be valid
            let verification = jwt::verify_and_refresh_tokens(
                Some(&initial_tokens.access_token),
                Some(&initial_tokens.refresh_token)
            );
            assert!(verification.is_ok(), "Initial tokens should be valid");
            assert!(verification.unwrap().is_none(), "No refresh needed for valid tokens");
        
            // Create an expired token using current time - 1 hour
            let jwt_secret = env::var("JWT_SECRET").unwrap();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as usize;
            
            let expired_claims = TokenClaims {
                sub: "test_user".to_string(),
                exp: now - 3600, // 1 hour ago
                iat: now - 7200, // 2 hours ago
                token_type: TokenType::Access,
            };
            
            let expired_token = jsonwebtoken::encode(
                &jsonwebtoken::Header::default(),
                &expired_claims,
                &jsonwebtoken::EncodingKey::from_secret(jwt_secret.as_bytes())
            ).expect("Failed to create expired token");
        
            // Test with expired access token but valid refresh token
            let expired_verification = jwt::verify_and_refresh_tokens(
                Some(&expired_token),
                Some(&initial_tokens.refresh_token)
            );
            assert!(expired_verification.is_ok(), "Should succeed with valid refresh token");
            let refreshed = expired_verification.unwrap();
            assert!(refreshed.is_some(), "Should get new tokens");
            
            let new_tokens = refreshed.unwrap();
            
            // Decode and verify the new access token
            let new_claims = jsonwebtoken::decode::<TokenClaims>(
                &new_tokens.access_token,
                &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes()),
                &jsonwebtoken::Validation::default()
            ).expect("Should decode new access token").claims;
        
            // Verify the new token properties
            assert_eq!(new_claims.sub, "test_user", "Subject should remain the same");
            assert_eq!(new_claims.token_type, TokenType::Access, "Token type should be Access");
            assert!(new_claims.exp > now, "New token should expire in the future");
            assert!(new_claims.iat >= now - 1, "New token should be issued around current time");
            assert!(new_claims.exp > new_claims.iat, "Expiration should be after issued time");
        
            // Try with invalid refresh token
            let invalid_verification = jwt::verify_and_refresh_tokens(
                Some(&expired_token),
                Some("invalid.refresh.token")
            );
            assert!(invalid_verification.is_err(), "Should fail with invalid refresh token");
            
            log::debug!("Token refresh flow test completed successfully");
        }

        #[tokio::test]
        async fn test_verify_invalid_tokens() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;

            log::debug!("Testing invalid token verification");

            // Test with completely invalid tokens
            let result = jwt::verify_and_refresh_tokens(
                Some("invalid.access.token"),
                Some("invalid.refresh.token")
            );
            assert!(result.is_err(), "Invalid tokens should fail verification");

            // Test with missing tokens
            let _missing = jwt::verify_and_refresh_tokens(None, None);
            assert!(result.is_err(), "Missing tokens should fail verification");

            log::debug!("Invalid token verification test completed");
        }
    }

    mod admin_login_tests {
        use super::*;
        use crate::auth::secure::verify_password;

        #[tokio::test]
        async fn test_verify_password_directly() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;

            let stored_hash = env::var("ADMIN_PASSWORD_HASH").expect("Hash should be set");
            log::debug!("Testing direct password verification");
            
            let result = verify_password("test_password", &stored_hash);
            log::debug!("Direct verification result: {result:?}");
            
            assert!(result.is_ok(), "Password verification should not error");
            assert!(result.unwrap(), "Password should verify correctly");
        }

        #[tokio::test]
        async fn test_successful_login() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;

            log::debug!("Testing successful login flow");

            let result = jwt::authenticate_admin("test_admin", "test_password").await;
            log::debug!("Authentication result: {result:?}");

            assert!(result.is_ok(), "Authentication should succeed");
            assert!(result.unwrap(), "Authentication should return true");
        }

        #[tokio::test]
        async fn test_failed_login_wrong_password() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;

            log::debug!("Testing login with wrong password");

            let result = jwt::authenticate_admin("test_admin", "wrong_password").await;
            log::debug!("Wrong password test result: {result:?}");
            
            assert!(result.is_ok(), "Authentication should process without error");
            assert!(!result.unwrap(), "Authentication should return false for wrong password");
        }

        #[tokio::test]
        async fn test_missing_env_vars() {
            let _lock = ENV_MUTEX.lock().await;
            log::debug!("Testing login with missing environment variables");

            let _guard = EnvVarGuard::new(vec![
                "JWT_SECRET".to_string(),
                "ADMIN_USERNAME".to_string(),
                "ADMIN_PASSWORD_HASH".to_string(),
            ]);

            let result = jwt::authenticate_admin("test_admin", "test_password").await;
            assert!(result.is_err(), "Authentication should fail with missing env vars");

            match result {
                Err(AuthError::MissingEnvironmentVar(_)) => (),
                other => panic!("Expected MissingEnvironmentVar error, got {other:?}"),
            }

            log::debug!("Missing environment variables test completed");
        }
    }

    mod cookie_tests {
        use super::*;
        use crate::auth::types::{AuthResponse, ACCESS_COOKIE_NAME, REFRESH_COOKIE_NAME};
        
        #[tokio::test]
        async fn test_auth_cookie_creation() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;
    
            log::debug!("Testing auth cookie creation");
    
            let auth_response = AuthResponse {
                access_token: "test.access.token".to_string(),
                refresh_token: "test.refresh.token".to_string(),
                access_expires_in: 900,
                refresh_expires_in: 604800,
            };
    
            let cookies = jwt::create_auth_cookies(&auth_response);
            assert_eq!(cookies.len(), 2, "Should create two cookies");
    
            let access_cookie = cookies.iter().find(|c| c.name() == ACCESS_COOKIE_NAME)
                .expect("Should have access token cookie");
            let refresh_cookie = cookies.iter().find(|c| c.name() == REFRESH_COOKIE_NAME)
                .expect("Should have refresh token cookie");
    
            // Verify cookie properties
            for cookie in [access_cookie, refresh_cookie] {
                assert!(cookie.http_only().unwrap_or(false), "Cookie should be HTTP only");
                assert!(cookie.secure().unwrap_or(false), "Cookie should be secure");
                assert_eq!(cookie.path().unwrap_or(""), "/", "Cookie should have root path");
                assert_eq!(
                    cookie.same_site().unwrap(),
                    cookie::SameSite::Strict,
                    "Cookie should have strict same-site policy"
                );
            }
    
            // Verify specific cookie values
            assert_eq!(access_cookie.value(), "test.access.token", "Access token value should match");
            assert_eq!(refresh_cookie.value(), "test.refresh.token", "Refresh token value should match");
    
            log::debug!("Auth cookie creation test completed");
        }
    
        #[tokio::test]
        async fn test_cookie_expiration() {
            let _lock = ENV_MUTEX.lock().await;
            initialize().await;
    
            log::debug!("Testing cookie expiration times");
    
            let auth_response = AuthResponse {
                access_token: "test.access.token".to_string(),
                refresh_token: "test.refresh.token".to_string(),
                access_expires_in: 900,      // 15 minutes
                refresh_expires_in: 604800,  // 7 days
            };
    
            let cookies = jwt::create_auth_cookies(&auth_response);
            
            let access_cookie = cookies.iter().find(|c| c.name() == ACCESS_COOKIE_NAME)
                .expect("Should have access token cookie");
            let refresh_cookie = cookies.iter().find(|c| c.name() == REFRESH_COOKIE_NAME)
                .expect("Should have refresh token cookie");
    
            // Verify that cookies have expiration times set
            assert!(access_cookie.expires().is_some(), "Access cookie should have expiration");
            assert!(refresh_cookie.expires().is_some(), "Refresh cookie should have expiration");
    
            log::debug!("Cookie expiration test completed");
        }
    }
}
