use axum::{
    response::sse::{Event, Sse},
    extract::{Query, State, Extension},
    Json,
    http::StatusCode,
};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::convert::Infallible;
use tokio::sync::mpsc as tokio_mpsc;
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use log::debug;

use crate::{
    cancellable_sse::{create_cancellable_sse_stream, CancellableSseStream},
    state::AppState,
    types::StreamResponse,
    auth::Claims,
};

pub struct SseStream {
    pub receiver: tokio_mpsc::Receiver<Result<Event, Infallible>>,
}

impl Stream for SseStream {
    type Item = Result<Event, Infallible>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.receiver.poll_recv(cx)
    }
}

pub async fn create_stream(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<StreamResponse>, StatusCode> {
    let user_id = claims.user_id()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    debug!("Creating SSE stream for user: {user_id}");
    
    let stream_id = uuid::Uuid::new_v4().to_string();
    state.sse_state.register_stream(stream_id.clone());
    
    debug!("Created SSE stream: {stream_id} for user: {user_id}");
    
    Ok(Json(StreamResponse { stream_id }))
}

pub async fn send_message_stream_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Extension(claims): Extension<Claims>,
) -> Result<Sse<CancellableSseStream>, StatusCode> {
    let user_id = claims.user_id()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let stream_id = params
        .get("stream_id")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    let thread_id = params.get("thread_id")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?;
    let model = params.get("model")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?;
    let lab = params.get("lab")
        .cloned()
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    debug!("Starting message stream for user: {user_id} - thread: {thread_id}, model: {model}, lab: {lab}");
    
    let pool = state.pool.clone();

    let sse_stream = create_cancellable_sse_stream(state.sse_state, stream_id, move |tx, token| {
        let pool = pool.clone();
        let thread_id = thread_id.clone();
        let model = model.clone();
        let lab = lab.clone();

        async move {
            crate::components::chat::send_message_stream_with_project_cancellable(
                &pool,
                thread_id,
                model,
                lab,
                tx,
                token
            ).await
        }
    }).await;
    
    Ok(sse_stream)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleUpdate {
    pub thread_id: String,
    pub title: String,
    pub status: String,
}

impl TitleUpdate {
    pub fn into_event(self) -> Result<Event, Infallible> {
        Ok(Event::default()
            .event("title_update")
            .data(serde_json::to_string(&self).unwrap_or_default()))
    }
}

pub async fn title_updates_handler(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Sse<SseStream>, StatusCode> {
    let user_id = claims.user_id()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    debug!("Starting title updates stream for user: {user_id}");

    let (tx, rx) = tokio_mpsc::channel(100);

    {
        state.title_update_senders.insert(user_id, tx);
    }

    Ok(Sse::new(SseStream { receiver: rx }))
}
