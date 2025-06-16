//use axum::{
//    extract::{
//        ws::{Message, WebSocket, WebSocketUpgrade},
//        State,
//    },
//    response::IntoResponse,
//};
//use futures::{sink::SinkExt, stream::StreamExt};
//use crate::state::{AppState, DrawEvent};
//
//pub async fn drawing_ws_handler(
//    ws: WebSocketUpgrade,
//    State(state): State<AppState>,
//) -> impl IntoResponse {
//    {
//        let mut user_count = state.user_count.lock().unwrap();
//        *user_count += 1;
//        tracing::debug!("Drawing user connected! Total users: {}", user_count);
//    }
//    ws.on_upgrade(move |socket| handle_drawing_socket(socket, state))
//}
//
//async fn handle_drawing_socket(socket: WebSocket, state: AppState) {
//    let (mut sender, mut receiver) = socket.split();
//    let mut rx = state.drawing_tx.subscribe();
//
//    // task which forwards messages from the broadcast channel to this ws
//    let mut send_task = tokio::spawn(async move {
//        while let Ok(event) = rx.recv().await {
//            if let Ok(json) = serde_json::to_string(&event) {
//                if sender.send(Message::Text(json.into())).await.is_err() {
//                    break;
//                }
//            }
//        }
//    });
//
//    // task that processes incoming messages from this ws
//    let tx = state.drawing_tx.clone();
//    let mut recv_task = tokio::spawn(async move {
//        while let Some(Ok(msg)) = receiver.next().await {
//            match msg {
//                Message::Text(text) => {
//                    // parse incoming drawing event and broadcast to all connected clients
//                    match serde_json::from_str::<DrawEvent>(&text) {
//                        Ok(event) => {
//                            let _ = tx.send(event);
//                        },
//                        Err(e) => {
//                            tracing::error!("Failed to parse drawing event: {}", e);
//                        }
//                    }
//                },
//                Message::Close(_) => break,
//                _ => {}
//            }
//        }
//    });
//
//    tokio::select! {
//        _ = (& mut send_task) => recv_task.abort(),
//        _ = (& mut recv_task ) => send_task.abort(),
//    }
//
//    let mut user_count = state.user_count.lock().unwrap();
//    *user_count -= 1;
//    tracing::debug!("Drawing user disconnected! Total users: {}", user_count);
//}
