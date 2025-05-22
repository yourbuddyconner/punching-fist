use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use crate::{
    scheduler::TaskScheduler,
    Result,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct Alert {
    pub name: String,
    pub status: String,
    pub severity: String,
    pub description: String,
    pub labels: std::collections::HashMap<String, String>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(scheduler): State<Arc<Mutex<TaskScheduler>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, scheduler))
}

async fn handle_socket(mut socket: WebSocket, scheduler: Arc<Mutex<TaskScheduler>>) {
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                match serde_json::from_str::<Alert>(&text) {
                    Ok(alert) => {
                        info!("Received alert: {:?}", alert);
                        if let Err(e) = handle_alert(alert, scheduler.clone()).await {
                            error!("Error handling alert: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse alert: {}", e);
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            _ => continue,
        }
    }
}

async fn handle_alert(alert: Alert, scheduler: Arc<Mutex<TaskScheduler>>) -> Result<()> {
    let mut scheduler = scheduler.lock().await;
    scheduler.schedule_task(alert).await
} 