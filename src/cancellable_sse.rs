use axum::{
    body::Body,
    extract::{Query, State, Request},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use futures::stream::Stream;
use log::info;
use std::{
    collections::HashMap,
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{
    state::AppState,
    auth::get_user_id_from_request,
};

pub struct CancellableSseStream {
    receiver: mpsc::Receiver<Result<Event, Infallible>>,
    cancel_token: CancellationToken,
}

impl Stream for CancellableSseStream {
    type Item = Result<Event, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.cancel_token.is_cancelled() {
            return Poll::Ready(None);
        }
        self.receiver.poll_recv(cx)
    }
}

#[derive(Clone)]
pub struct SseState {
    cancel_tokens: Arc<dashmap::DashMap<String, CancellationToken>>,
}

impl Default for SseState {
    fn default() -> Self {
        Self {
            cancel_tokens: Arc::new(dashmap::DashMap::new()),
        }
    }
}

impl SseState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_stream(&self, id: String) -> CancellationToken {
        let token = CancellationToken::new();
        self.cancel_tokens.insert(id.clone(), token.clone());
        token
    }

    pub fn cancel_stream(&self, id: &str) {
        if let Some((_, token)) = self.cancel_tokens.remove(id) {
            token.cancel();
        }
    }
}

pub async fn create_cancellable_sse_stream<F, Fut>(
    state: SseState,
    stream_id: String,
    process_fn: F,
) -> Sse<CancellableSseStream>
where
    F: FnOnce(mpsc::Sender<Result<Event, Infallible>>, CancellationToken) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static,
{
    let (tx, rx) = mpsc::channel(100);
    let cancel_token = state.register_stream(stream_id);

    let task_token = cancel_token.clone();

    tokio::spawn(async move {
        let result = process_fn(tx, task_token).await;
        if let Err(e) = result {
            log::error!("Error in SSE stream: {e}");
        }
    });

    Sse::new(CancellableSseStream {
        receiver: rx,
        cancel_token,
    })
}

pub async fn cancel_stream(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    request: Request<Body>,
) -> Result<&'static str, StatusCode> {
    let user_id = get_user_id_from_request(&request)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    if let Some(stream_id) = params.get("stream_id") {
        info!("Cancelling stream: {stream_id} for user: {user_id}");
        state.sse_state.cancel_stream(stream_id);
        Ok("Stream cancelled")
    } else {
        Ok("No stream ID provided")
    }
}
