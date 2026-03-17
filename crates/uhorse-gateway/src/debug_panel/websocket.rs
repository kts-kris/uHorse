//! WebSocket handler for real-time debug updates

use super::{DebugEvent, DebugPanelState};
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;

/// Handle WebSocket connection
pub async fn handle_websocket(socket: WebSocket, state: Arc<DebugPanelState>) {
    let (mut sender, mut receiver) = socket.split();

    // Subscribe to events
    let mut rx = state.event_tx.subscribe();

    // Spawn task to send events
    let send_task = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            let json = serde_json::to_string(&event).unwrap_or_default();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                // Handle client commands
                if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                    handle_client_command(&state, &cmd).await;
                }
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}

/// Client commands
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ClientCommand {
    /// Subscribe to specific session
    SubscribeSession { session_id: String },
    /// Unsubscribe from session
    UnsubscribeSession { session_id: String },
    /// Filter events by type
    SetFilter { event_types: Vec<String> },
    /// Clear event history
    ClearHistory,
}

async fn handle_client_command(state: &DebugPanelState, cmd: &ClientCommand) {
    match cmd {
        ClientCommand::ClearHistory => {
            let mut events = state.events.write().await;
            events.clear();
        }
        _ => {
            // Other commands not yet implemented
        }
    }
}
