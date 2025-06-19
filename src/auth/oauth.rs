#[cfg(feature = "ssr")]
pub mod oauth_server {
    use axum::{
        extract::{Query, State},
        response::{IntoResponse, Redirect},
        http::HeaderMap,
    };
    use axum_extra::extract::cookie::{Cookie, SameSite};
    use serde::{Deserialize, Serialize};
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use log::{debug, info, error};
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use sha2::{Digest, Sha256};
    use rand::{thread_rng, Rng};
    
    use crate::state::AppState;
    use crate::models::users::{User, NewUser, CreateUserView};
    use crate::schema::users;

    #[derive(Serialize, Deserialize, Clone)]
    pub struct OAuthState {
        pub provider: String,
        pub verifier: String,
        pub return_url: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct OAuthCallback {
        pub code: String,
        pub state: String,
    }

    #[derive(Deserialize)]
    pub struct GoogleUserInfo {
        pub id: String,
        pub email: Option<String>,
        pub name: Option<String>,
        pub picture: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct DiscordUserInfo {
        pub id: String,
        pub username: Option<String>,
        pub email: Option<String>,
        pub avatar: Option<String>,
        pub discriminator: Option<String>,
    }

    pub async fn google_login(State(state): State<AppState>) -> impl IntoResponse {
        let scopes = vec!["openid".to_string(), "email".to_string(), "profile".to_string()];
        oauth_login("google", scopes, state).await
    }

    pub async fn discord_login(State(state): State<AppState>) -> impl IntoResponse {
        let scopes = vec!["identify".to_string(), "email".to_string()];
        oauth_login("discord", scopes, state).await
    }

    async fn oauth_login(
        provider: &str,
        scopes: Vec<String>,
        state: AppState,
    ) -> impl IntoResponse {

        let oauth_state = uuid::Uuid::new_v4().to_string();
        let (code_verifier, code_challenge) = generate_pkce(); 
        
        let session_state = OAuthState {
            provider: provider.to_string(),
            verifier: code_verifier, 
            return_url: None,
        };

        state.oauth_states.insert(oauth_state.clone(), session_state);

        let auth_url = match provider {
            "google" => {
                let scope = scopes.join(" ");
                format!(
                    "https://accounts.google.com/o/oauth2/auth?client_id={}&redirect_uri={}&scope={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256",
                    std::env::var("GOOGLE_CLIENT_ID").expect("GOOGLE_CLIENT_ID must be set"),
                    urlencoding::encode(&std::env::var("GOOGLE_REDIRECT_URL")
                        .unwrap_or_else(|_| "http://localhost:3000/auth/google-callback".to_string())),
                    urlencoding::encode(&scope),
                    urlencoding::encode(&oauth_state),
                    urlencoding::encode(&code_challenge)
                )
            }
            "discord" => {
                let scope = scopes.join("%20");
                format!(
                    "https://discord.com/api/oauth2/authorize?client_id={}&redirect_uri={}&scope={}&response_type=code&state={}",
                    std::env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID must be set"),
                    urlencoding::encode(&std::env::var("DISCORD_REDIRECT_URL")
                        .unwrap_or_else(|_| "http://localhost:3000/auth/discord/callback".to_string())),
                    scope,
                    urlencoding::encode(&oauth_state)
                )
            }
            _ => {
                error!("Unsupported OAuth provider: {provider}");
                return (axum::http::StatusCode::BAD_REQUEST, "Unsupported provider").into_response();
            }
        };

        Redirect::to(&auth_url).into_response()
    }

    fn generate_pkce() -> (String, String) {
        // Generate a random 32-byte array for the verifier
        let mut verifier_bytes = [0u8; 32];
        thread_rng().fill(&mut verifier_bytes);
        
        // Base64url-encode the verifier (without padding)
        let code_verifier = URL_SAFE_NO_PAD.encode(&verifier_bytes);
        
        // Create SHA256 hash of the verifier
        let mut hasher = Sha256::new();
        hasher.update(&code_verifier);
        let challenge_bytes = hasher.finalize();
        
        // Base64url-encode the challenge (without padding)
        let code_challenge = URL_SAFE_NO_PAD.encode(&challenge_bytes);
        
        (code_verifier, code_challenge)
    }

    pub async fn google_callback(
        State(state): State<AppState>,
        Query(params): Query<OAuthCallback>,
    ) -> impl IntoResponse {
        oauth_callback("google", state, params).await
    }

    pub async fn discord_callback(
        State(state): State<AppState>,
        Query(params): Query<OAuthCallback>,
    ) -> impl IntoResponse {
        oauth_callback("discord", state, params).await
    }

    async fn exchange_code_for_token(
        provider: &str,
        code: &str,
        verifier: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        
        let (token_url, client_id, client_secret, redirect_uri) = match provider {
            "google" => (
                "https://oauth2.googleapis.com/token",
                std::env::var("GOOGLE_CLIENT_ID")?,
                std::env::var("GOOGLE_CLIENT_SECRET")?,
                std::env::var("GOOGLE_REDIRECT_URL")
                    .unwrap_or_else(|_| "http://localhost:3000/auth/google-callback".to_string()),
            ),
            "discord" => (
                "https://discord.com/api/oauth2/token",
                std::env::var("DISCORD_CLIENT_ID")?,
                std::env::var("DISCORD_CLIENT_SECRET")?,
                std::env::var("DISCORD_REDIRECT_URL")
                    .unwrap_or_else(|_| "http://localhost:3000/auth/discord/callback".to_string()),
            ),
            _ => return Err("Unsupported provider".into()),
        };
    
        debug!("Exchanging code for token - Provider: {}, Client ID: {}", provider, &client_id[..8]);
    
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
            ("redirect_uri", &redirect_uri),
        ];
    
        // Google uses PKCE
        if provider == "google" {
            params.push(("code_verifier", verifier));
        }
    
        debug!("Token request params (excluding secrets): grant_type=authorization_code, redirect_uri={redirect_uri}");
    
        let response = client
            .post(token_url)
            .form(&params)
            .send()
            .await?;
    
        let status = response.status();
        let response_text = response.text().await?;
        
        debug!("Token response status: {status}");
        debug!("Token response body: {response_text}");
    
        if !status.is_success() {
            return Err(format!("Token exchange failed with status {status}: {response_text}").into());
        }
    
        #[derive(serde::Deserialize)]
        struct TokenResponse {
            access_token: String,
            #[serde(default)]
            error: Option<String>,
            #[serde(default)]
            error_description: Option<String>,
        }
    
        let token_data: TokenResponse = serde_json::from_str(&response_text)?;
        
        if let Some(error) = token_data.error {
            return Err(format!("OAuth error: {} - {}", error, token_data.error_description.unwrap_or_default()).into());
        }
    
        debug!("Successfully obtained access token");
        Ok(token_data.access_token)
    }

    async fn get_user_info(provider: &str, token: &str) -> Result<CreateUserView, Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();
        
        debug!("Fetching user info from {provider} with token");
        
        match provider {
            "google" => {
                let response = client
                    .get("https://www.googleapis.com/oauth2/v2/userinfo")
                    .bearer_auth(token)
                    .send()
                    .await?;
    
                let status = response.status();
                let response_text = response.text().await?;
                
                debug!("Google user info response status: {status}");
                debug!("Google user info response: {response_text}");
    
                if !status.is_success() {
                    return Err(format!("Failed to get Google user info: {response_text}").into());
                }
    
                let resp: GoogleUserInfo = serde_json::from_str(&response_text)?;
                debug!("Parsed Google user info: ID={}, Email={:?}", resp.id, resp.email);
    
                Ok(CreateUserView {
                    external_id: resp.id,
                    provider: "google".to_string(),
                    email: resp.email,
                    username: None,
                    display_name: resp.name,
                    avatar_url: resp.picture,
                })
            }
            "discord" => {
                let response = client
                    .get("https://discord.com/api/v10/users/@me")
                    .bearer_auth(token)
                    .send()
                    .await?;
    
                let status = response.status();
                let response_text = response.text().await?;
                
                debug!("Discord user info response status: {status}");
                debug!("Discord user info response: {response_text}");
    
                if !status.is_success() {
                    return Err(format!("Failed to get Discord user info: {response_text}").into());
                }
    
                let resp: DiscordUserInfo = serde_json::from_str(&response_text)?;
                debug!("Parsed Discord user info: ID={}, Username={:?}", resp.id, resp.username);
    
                let avatar_url = resp.avatar.as_ref().map(|avatar| {
                    format!("https://cdn.discordapp.com/avatars/{}/{}.png", resp.id, avatar)
                });
    
                let username_for_formatting = resp.username.clone();
                let username = username_for_formatting.clone().or_else(|| {
                    resp.discriminator.as_ref().map(|disc| {
                        format!("{}#{}", username_for_formatting.unwrap_or_default(), disc)
                    })
                });
    
                Ok(CreateUserView {
                    external_id: resp.id,
                    provider: "discord".to_string(),
                    email: resp.email,
                    username,
                    display_name: resp.username,
                    avatar_url,
                })
            }
            _ => Err("Unsupported provider".into()),
        }
    }
    
    async fn oauth_callback(
        provider: &str,
        app_state: AppState,
        params: OAuthCallback,
    ) -> impl IntoResponse {
        debug!("OAuth callback received for provider: {provider}");
        debug!("Callback params - code length: {}, state: {}", params.code.len(), params.state);
    
        let oauth_state = match app_state.oauth_states.get(&params.state) {
            Some(state_ref) => {
                debug!("Found OAuth state for: {}", params.state);
                state_ref.value().clone()
            },
            None => {
                error!("OAuth state not found for state: {}", params.state);
                error!("Available states: {:?}", app_state.oauth_states.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>());
                return Redirect::to("/admin?error=invalid_state").into_response();
            }
        };
    
        app_state.oauth_states.remove(&params.state);
        debug!("Cleaned up OAuth state");
    
        debug!("Starting token exchange...");
        let token = match exchange_code_for_token(provider, &params.code, &oauth_state.verifier).await {
            Ok(token) => {
                debug!("Token exchange successful");
                token
            },
            Err(e) => {
                error!("Token exchange failed: {e:?}");
                return Redirect::to(&format!("/admin?error=token_exchange_failed&details={}", urlencoding::encode(&e.to_string()))).into_response();
            }
        };
    
        debug!("Fetching user info...");
        let user_info = match get_user_info(provider, &token).await {
            Ok(info) => {
                debug!("User info fetched successfully for external_id: {}", info.external_id);
                info
            },
            Err(e) => {
                error!("Failed to get user info: {e:?}");
                return Redirect::to(&format!("/admin?error=user_info_failed&details={}", urlencoding::encode(&e.to_string()))).into_response();
            }
        };
    
        debug!("Upserting user in database...");
        let user = match upsert_user(&app_state, user_info).await {
            Ok(user) => {
                debug!("User upserted successfully with ID: {}", user.id);
                user
            },
            Err(e) => {
                error!("Failed to create/update user: {e:?}");
                return Redirect::to(&format!("/admin?error=db_error&details={}", urlencoding::encode(&e.to_string()))).into_response();
            }
        };
    
        debug!("Creating JWT token...");
        let jwt_token = match crate::auth::create_jwt_token(user.id) {
            Ok(token) => {
                debug!("JWT token created successfully");
                token
            },
            Err(e) => {
                error!("Failed to create JWT: {e:?}");
                return Redirect::to(&format!("/admin?error=jwt_error&details={}", urlencoding::encode(&e.to_string()))).into_response();
            }
        };
    
        debug!("Setting auth cookie and redirecting to admin panel");
        let cookie = Cookie::build(("auth_token", jwt_token))
            .path("/")
            .secure(true)
            .http_only(true)
            .same_site(SameSite::Lax)
            .build();
    
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::SET_COOKIE,
            cookie.to_string().parse().unwrap(),
        );
    
        (headers, Redirect::to("/admin-panel")).into_response()
    }

    async fn upsert_user(
        app_state: &AppState,
        user_info: CreateUserView,
    ) -> Result<User, Box<dyn std::error::Error>> {
        let mut conn = app_state.pool.get().await?;

        let existing_user = users::table
            .filter(users::external_id.eq(&user_info.external_id))
            .filter(users::provider.eq(&user_info.provider))
            .first::<User>(&mut conn)
            .await
            .optional()?;

        if let Some(mut user) = existing_user {
            diesel::update(users::table.find(user.id))
                .set((
                    users::email.eq(&user_info.email),
                    users::username.eq(&user_info.username),
                    users::display_name.eq(&user_info.display_name),
                    users::avatar_url.eq(&user_info.avatar_url),
                    users::updated_at.eq(diesel::dsl::now),
                ))
                .execute(&mut conn)
                .await?;

            user.email = user_info.email;
            user.username = user_info.username;
            user.display_name = user_info.display_name;
            user.avatar_url = user_info.avatar_url;

            Ok(user)
        } else {
            let new_user: NewUser = user_info.into();
            let user = diesel::insert_into(users::table)
                .values(&new_user)
                .get_result::<User>(&mut conn)
                .await?;

            info!("Created new user: {:?}", user.id);
            Ok(user)
        }
    }
}

#[cfg(feature = "ssr")]
pub use oauth_server::*;
