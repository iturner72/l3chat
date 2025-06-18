use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::extract::FromRef;
        use axum::response::sse::Event;
        use dashmap::DashMap;
        use leptos::prelude::LeptosOptions;
        use std::convert::Infallible;
        use std::sync::Arc;
        use tokio::sync::mpsc;

        use crate::cancellable_sse::SseState;
        use crate::database::db::DbPool;
        use crate::auth::oauth::OAuthState;

        pub type TitleUpdateSender = mpsc::Sender<Result<Event, Infallible>>;
        pub type TitleUpdateSenders = Arc<DashMap<i32, TitleUpdateSender>>;

        #[derive(FromRef, Clone)]
        pub struct AppState {
            pub leptos_options: LeptosOptions,
            pub pool: DbPool,
            pub sse_state: SseState,
            pub oauth_states: Arc<DashMap<String, OAuthState>>,
            pub title_update_senders: TitleUpdateSenders,
        }

        impl AppState {
            pub fn new(leptos_options: LeptosOptions, pool: DbPool) -> Self {
                Self {
                    leptos_options,
                    pool,
                    sse_state: SseState::new(),
                    oauth_states: Arc::new(DashMap::new()),
                    title_update_senders: Arc::new(DashMap::new()),
                }
            }
        }
    }
}
