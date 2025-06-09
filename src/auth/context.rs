use leptos::{prelude::*, task::spawn_local};

use crate::auth::{get_current_user, verify_token};
use crate::models::users::UserView;

#[derive(Clone)]
pub struct AuthContext {
    pub is_authenticated: ReadSignal<bool>,
    pub current_user: ReadSignal<Option<UserView>>,
    pub is_loading: ReadSignal<bool>,
    pub refresh: WriteSignal<u32>,
}

impl AuthContext {
    pub fn refresh_auth(&self) {
        self.refresh.update(|v| *v = (*v + 1) % 1000);
    }
}

#[component]
pub fn AuthProvider(children: Children) -> impl IntoView {
    let (is_authenticated, set_is_authenticated) = signal(false);
    let (current_user, set_current_user) = signal(None::<UserView>);
    let (is_loading, set_is_loading) = signal(true);
    let (refresh, set_refresh) = signal(0u32);

    let auth_context = AuthContext {
        is_authenticated,
        current_user,
        is_loading,
        refresh: set_refresh,
    };

    Effect::new(move |_| {
        refresh.get();
        spawn_local(async move {
            set_is_loading(true);

            match verify_token().await {
                Ok(is_valid) => {
                    set_is_authenticated(is_valid);
                    if is_valid {
                        if let Ok(Some(user)) = get_current_user().await {
                            set_current_user(Some(user));
                        }
                    } else {
                        set_current_user(None);
                    }
                }
                Err(_) => {
                    set_is_authenticated(false);
                    set_current_user(None);
                }
            }
            set_is_loading(false);
        });
    });

    provide_context(auth_context.clone());

    view! { {children()} }
}
