use axum::{
    response::sse::{Event, Sse},
    extract::{Query, State, Extension},
    Json,
    http::StatusCode,
};
use std::collections::HashMap;
use std::convert::Infallible;
use tokio::sync::mpsc as tokio_mpsc;
use futures::stream::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use log::info;

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
    
    info!("Creating SSE stream for user: {user_id}");
    
    let stream_id = uuid::Uuid::new_v4().to_string();
    state.sse_state.register_stream(stream_id.clone());
    
    info!("Created SSE stream: {stream_id} for user: {user_id}");
    
    Ok(Json(StreamResponse { stream_id }))
}

pub async fn embeddings_generation_handler(
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
    
    info!("Starting embeddings generation for user: {user_id} with stream: {stream_id}");
    
    let sse_stream = create_cancellable_sse_stream(
        state.sse_state,
        stream_id,
        |tx, token| async move {
            crate::embedding_service::embeddings::generate_embeddings(tx, token).await
        }
    ).await;
    
    Ok(sse_stream)
}

pub async fn local_embeddings_generation_handler(
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
    
    info!("Starting local embeddings generation for user: {user_id} with stream: {stream_id}");
    
    let sse_stream = create_cancellable_sse_stream(
        state.sse_state,
        stream_id,
        |tx, token| async move {
            crate::embeddings_service::embeddings_local::generate_local_embeddings(tx, token).await
        }
    ).await;
    
    Ok(sse_stream)
}

pub async fn send_message_stream_handler(
    State(app_state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
    Extension(claims): Extension<Claims>,
) -> Result<Sse<SseStream>, StatusCode> {
    let user_id = claims.user_id()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    let thread_id = params.get("thread_id")
        .ok_or(StatusCode::BAD_REQUEST)?;
    let model = params.get("model")
        .ok_or(StatusCode::BAD_REQUEST)?;
    let lab = params.get("lab")
        .ok_or(StatusCode::BAD_REQUEST)?;
    
    info!("Starting message stream for user: {user_id} - thread: {thread_id}, model: {model}, lab: {lab}");
    
    let (tx, rx) = tokio_mpsc::channel(100);
    
    let pool = app_state.pool.clone();
    let thread_id = thread_id.clone();
    let model = model.clone();
    let lab = lab.clone();
    
    tokio::spawn(async move {
        crate::components::chat::send_message_stream(&pool, thread_id, model, lab, tx).await;
    });
    
    Ok(Sse::new(SseStream { receiver: rx }))
}
