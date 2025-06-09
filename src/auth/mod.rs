pub mod auth_components;
pub mod context;
#[cfg(feature = "ssr")]
pub mod oauth;
#[cfg(feature = "ssr")]
pub mod secure;
#[cfg(feature = "ssr")]
pub mod server;
mod types;

pub use auth_components::*;
pub use types::*;

#[cfg(feature = "ssr")]
pub use server::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

#[cfg(feature = "ssr")]
impl Claims {
    pub fn user_id(&self) -> Result<i32, std::num::ParseIntError> {
        self.sub.parse()
    }
}

#[cfg(feature = "ssr")]
pub fn create_jwt_token(user_id: i32) -> Result<String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, Header, EncodingKey};
    
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    let now = chrono::Utc::now();
    
    let claims = Claims {
        sub: user_id.to_string(),
        exp: (now + chrono::Duration::hours(24)).timestamp(),
        iat: now.timestamp(),
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

#[cfg(feature = "ssr")]
pub fn verify_jwt_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{decode, DecodingKey, Validation};
    
    let secret = std::env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;
    
    Ok(token_data.claims)
}

#[leptos::server(VerifyToken, "/api")]
pub async fn verify_token() -> Result<bool, leptos::server_fn::ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use axum_extra::extract::cookie::CookieJar;
        use leptos_axum::extract;
        
        let jar = extract::<CookieJar>().await
            .map_err(|e| leptos::server_fn::ServerFnError::new(format!("Cookie jar error: {e}")))?;
        
        if let Some(cookie) = jar.get("auth_token") {
            match verify_jwt_token(cookie.value()) {
                Ok(_) => Ok(true),
                Err(_) => Ok(false),
            }
        } else {
            Ok(false)
        }
    }
}

#[leptos::server(GetCurrentUser, "/api")]
pub async fn get_current_user() -> Result<Option<crate::models::users::UserView>, leptos::server_fn::ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use axum_extra::extract::cookie::CookieJar;
        use leptos_axum::extract;
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use crate::state::AppState;
        use crate::models::users::User;
        use crate::schema::users;
        
        let jar = extract::<CookieJar>().await
            .map_err(|e| leptos::server_fn::ServerFnError::new(format!("Cookie jar error: {e}")))?;
        
        if let Some(cookie) = jar.get("auth_token") {
            if let Ok(claims) = verify_jwt_token(cookie.value()) {
                let user_id: i32 = claims.sub.parse()
                    .map_err(|_| leptos::server_fn::ServerFnError::new("Invalid user ID in token"))?;
                
                let app_state = leptos::context::use_context::<AppState>()
                    .ok_or_else(|| leptos::server_fn::ServerFnError::new("App state not found"))?;
                
                let mut conn = app_state.pool.get().await
                    .map_err(|e| leptos::server_fn::ServerFnError::new(format!("Database connection error: {e}")))?;
                
                let user = users::table
                    .find(user_id)
                    .first::<User>(&mut conn)
                    .await
                    .optional()
                    .map_err(|e| leptos::server_fn::ServerFnError::new(format!("Database query error: {e}")))?;
                
                Ok(user.map(|u| u.into()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
    
    #[cfg(not(feature = "ssr"))]
    {
        // Client-side implementation - return None since we can't verify JWT on client
        Ok(None)
    }
}

#[leptos::server(Logout, "/api")]
pub async fn logout() -> Result<(), leptos::server_fn::ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use axum_extra::extract::cookie::Cookie;
        use leptos_axum::ResponseOptions;
        use http::{HeaderName, HeaderValue};
        
        let response_options = leptos::context::use_context::<ResponseOptions>()
            .ok_or_else(|| leptos::server_fn::ServerFnError::new("Response options not found"))?;
        
        let cookie = Cookie::build(("auth_token", ""))
            .path("/")
            .max_age(cookie::time::Duration::seconds(-1))
            .build();
        
        let cookie_value = HeaderValue::from_str(&cookie.to_string())
            .map_err(|e| leptos::server_fn::ServerFnError::new(format!("Cookie header error: {e}")))?;
        
        response_options.insert_header(HeaderName::from_static("set-cookie"), cookie_value);
    }
    
    Ok(())
}
