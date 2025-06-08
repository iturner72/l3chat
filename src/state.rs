use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "ssr")] {
        use axum::extract::FromRef;
        use tokio::sync::broadcast;
        use std::sync::{Arc,Mutex};
        use leptos::prelude::LeptosOptions;
        use serde::{Serialize, Deserialize};

        use crate::cancellable_sse::SseState;

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

        #[derive(FromRef, Clone)]
        pub struct AppState {
            pub leptos_options: LeptosOptions,
            pub sse_state: SseState,
            pub drawing_tx: broadcast::Sender<DrawEvent>,
            pub user_count: Arc<Mutex<usize>>,

        }

        impl AppState {
            pub fn new(leptos_options: LeptosOptions) -> Self {
                let (drawing_tx, _) = broadcast::channel(100);
                Self {
                    leptos_options,
                    sse_state: SseState::new(),
                    drawing_tx,
                    user_count: Arc::new(Mutex::new(0))
                }
            }
        }
    }
}
