use leptos::{prelude::*, task::spawn_local};
use web_sys::window;

#[cfg(feature = "ssr")]
const DARK_MODE_COOKIE: &str = "bb_dark_mode";

#[server(SetDarkModeCookie, "/api")]
pub async fn set_dark_mode_cookie(is_dark: bool) -> Result<(), ServerFnError> {
    use crate::auth::{AuthError, to_server_error};
    use axum_extra::extract::cookie::{Cookie, SameSite};
    use cookie::time;
    use http::{HeaderName, HeaderValue};

    let cookie = Cookie::build((DARK_MODE_COOKIE, is_dark.to_string()))
        .path("/")
        .secure(true)
        .http_only(false)
        .same_site(SameSite::Lax)
        .expires(time::OffsetDateTime::now_utc() + time::Duration::days(365))
        .build();

    let response_options = use_context::<leptos_axum::ResponseOptions>()
        .expect("response options not found");

    let cookie_value = HeaderValue::from_str(&cookie.to_string())
        .map_err(|e| to_server_error(AuthError::CookieError(e.to_string())))?;

    response_options.insert_header(HeaderName::from_static("set-cookie"), cookie_value);

    Ok(())
}

#[server(GetDarkModeCookie, "/api")]
pub async fn get_dark_mode_cookie() -> Result<Option<bool>, ServerFnError> {
    use crate::auth::{AuthError, to_server_error};
    use leptos_axum::extract;
    use axum_extra::extract::cookie::CookieJar;

    let cookie_jar = extract::<CookieJar>().await
        .map_err(|e| to_server_error(AuthError::CookieError(e.to_string())))?;

    Ok(cookie_jar
        .get(DARK_MODE_COOKIE)
        .and_then(|cookie| cookie.value().parse().ok()))
}

#[component]
pub fn DarkModeToggle() -> impl IntoView {
    let (is_dark, set_is_dark) = signal(false);
    let set_cookie_action = ServerAction::<SetDarkModeCookie>::new();

    Effect::new(move |_| {
        spawn_local(async move {
            if let Ok(Some(dark_mode)) = get_dark_mode_cookie().await {
                set_is_dark.set(dark_mode);
                apply_dark_mode(dark_mode);
            }
        });
    });

    let toggle_dark_mode = move |_| {
        let new_state = !is_dark.get();
        set_is_dark.set(new_state);
        apply_dark_mode(new_state);

        set_cookie_action.dispatch(SetDarkModeCookie { is_dark: new_state });
    };

    view! {
        <button on:click=toggle_dark_mode>
            {move || if is_dark.get() { "🌞" } else { "🌙" }}
        </button>
    }
}

fn apply_dark_mode(is_dark: bool) {
    if let Some(window) = window() {
        if let Some(document) = window.document() {
            if let Some(body) = document.body() {
                let _ = if is_dark {
                    body.class_list().add_1("dark")
                } else {
                    body.class_list().remove_1("dark")
                };
            }
        }
    }
}
