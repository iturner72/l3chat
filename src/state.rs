use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::extract::FromRef;
        use axum::response::sse::Event;
        use dashmap::DashMap;
        use leptos::prelude::LeptosOptions;
        use serde::{Serialize, Deserialize};
        use std::convert::Infallible;
        use std::sync::{Arc,Mutex};
        use tokio::sync::{broadcast, mpsc};

        use crate::cancellable_sse::SseState;
        use crate::database::db::DbPool;
        use crate::auth::oauth::OAuthState;

        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct DrawEvent {
            pub event_type: String,
            pub x: f64,
            pub y: f64,
            pub prev_x: Option<f64>,
            pub prev_y: Option<f64>,
            pub color: String,
            pub brush_size: u32,
            pub room_id: String,
            pub user_id: String,
        }

        pub type TitleUpdateSender = mpsc::Sender<Result<Event, Infallible>>;
        pub type TitleUpdateSenders = Arc<DashMap<i32, TitleUpdateSender>>;

        #[derive(FromRef, Clone)]
        pub struct AppState {
            pub leptos_options: LeptosOptions,
            pub pool: DbPool,
            pub sse_state: SseState,
            pub drawing_tx: broadcast::Sender<DrawEvent>,
            pub user_count: Arc<Mutex<usize>>,
            pub oauth_states: Arc<DashMap<String, OAuthState>>,
            pub title_update_senders: TitleUpdateSenders,
        }

        impl AppState {
            pub fn new(leptos_options: LeptosOptions, pool: DbPool) -> Self {
                let (drawing_tx, _) = broadcast::channel(100);
                Self {
                    leptos_options,
                    pool,
                    sse_state: SseState::new(),
                    drawing_tx,
                    user_count: Arc::new(Mutex::new(0)),
                    oauth_states: Arc::new(DashMap::new()),
                    title_update_senders: Arc::new(DashMap::new()),
                }
            }
        }
    }
}
