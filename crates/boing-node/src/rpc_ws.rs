//! WebSocket **`GET /ws`** — subscribe to committed chain tip events (**newHeads**).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::Json;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::rpc::RpcState;

struct WsConnectionPermit(Arc<AtomicUsize>);

impl Drop for WsConnectionPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

pub async fn ws_new_heads_upgrade(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<RpcState>,
) -> impl IntoResponse {
    let head = state.head_broadcast.clone();
    let max = state.ws_max_connections;
    let active = state.ws_active.clone();
    let permit = if max > 0 {
        let prev = active.fetch_add(1, Ordering::SeqCst);
        if prev >= max {
            active.fetch_sub(1, Ordering::SeqCst);
            state
                .rpc_metrics
                .websocket_cap_rejects
                .fetch_add(1, Ordering::Relaxed);
            let rid = headers.get("x-request-id").and_then(|v| v.to_str().ok());
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(
                    CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                )],
                Json(serde_json::json!({
                    "code": "websocket_max_connections",
                    "message": "WebSocket subscriber limit reached for this node (BOING_RPC_WS_MAX_CONNECTIONS).",
                    "request_id": rid,
                })),
            )
                .into_response();
        }
        Some(WsConnectionPermit(active))
    } else {
        None
    };
    ws.on_upgrade(move |socket| ws_new_heads_socket(socket, head, permit))
}

async fn ws_new_heads_socket(
    socket: WebSocket,
    head_broadcast: Option<broadcast::Sender<Value>>,
    permit: Option<WsConnectionPermit>,
) {
    let _permit = permit;
    let Some(tx) = head_broadcast else {
        return;
    };

    let (mut sender, mut receiver) = socket.split();

    let first = match receiver.next().await {
        Some(Ok(Message::Text(t))) => t,
        _ => {
            let _ = sender.close().await;
            return;
        }
    };

    let Ok(v) = serde_json::from_str::<Value>(&first) else {
        let _ = sender
            .send(Message::Text(
                json!({"type":"error","code":"invalid_json","message":"expected JSON text frame"})
                    .to_string(),
            ))
            .await;
        let _ = sender.close().await;
        return;
    };

    if v.get("type").and_then(|x| x.as_str()) != Some("subscribe")
        || v.get("channel").and_then(|x| x.as_str()) != Some("newHeads")
    {
        let _ = sender
            .send(Message::Text(
                json!({"type":"error","code":"invalid_subscribe","message":"first message must be {\"type\":\"subscribe\",\"channel\":\"newHeads\"}"}).to_string(),
            ))
            .await;
        let _ = sender.close().await;
        return;
    }

    if sender
        .send(Message::Text(
            json!({"type":"subscribed","channel":"newHeads"}).to_string(),
        ))
        .await
        .is_err()
    {
        return;
    }

    let mut sub = tx.subscribe();

    loop {
        tokio::select! {
            incoming = sub.recv() => {
                match incoming {
                    Ok(msg) => {
                        if sender.send(Message::Text(msg.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = receiver.next() => {
                match msg {
                    None => break,
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(Message::Ping(p))) => {
                        if sender.send(Message::Pong(p)).await.is_err() {
                            break;
                        }
                    }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}
