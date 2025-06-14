use cfg_if::cfg_if;

#[cfg(feature = "ssr")]
pub mod middleware {
    use axum::{
        extract::{State, Request},
        middleware::Next,
        response::{Response, IntoResponse},
        http::StatusCode,
    };
    use axum_extra::extract::CookieJar;
    use log::{debug, warn};
    
    use crate::state::AppState;
    use crate::auth::verify_jwt_token;

    /// Middleware that requires authentication via JWT token stored in cookies
    pub async fn require_auth_no_db(
        cookie_jar: CookieJar,
        State(_app_state): State<AppState>,
        request: Request,
        next: Next,
    ) -> Response {
        debug!(
            "Auth middleware - Processing request to: {} {}",
            request.method(),
            request.uri()
        );
    
        let auth_token = cookie_jar.get("auth_token").map(|c| c.value());
        
        debug!("Auth middleware - Found auth token: {}", auth_token.is_some());
    
        match auth_token {
            Some(token) => {
                match verify_jwt_token(token) {
                    Ok(claims) => {
                        debug!("Auth middleware - Token verified for user: {}", claims.sub);
                        
                        if let Ok(user_id) = claims.sub.parse::<i32>() {
                            debug!("Auth middleware - User {user_id} - skipping DB check");
                            
                            // Add user claims to request extensions for downstream handlers
                            let mut request = request;
                            request.extensions_mut().insert(claims);
                            
                            // Continue to the protected route
                            next.run(request).await
                        } else {
                            warn!("Auth middleware - Invalid user ID in token: {}", claims.sub);
                            StatusCode::UNAUTHORIZED.into_response()
                        }
                    }
                    Err(e) => {
                        debug!("Auth middleware - Token verification failed: {e:?}");
                        StatusCode::UNAUTHORIZED.into_response()
                    }
                }
            }
            None => {
                debug!("Auth middleware - No auth token found");
                StatusCode::UNAUTHORIZED.into_response()
            }
        }
    }
}

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::{
            body::Body,
            http::Request,
        };
        use crate::auth::Claims;
        
        pub fn get_user_id_from_request(request: &Request<Body>) -> Option<i32> {
            request.extensions()
                .get::<Claims>()
                .and_then(|claims| claims.sub.parse().ok())
        }
}}
