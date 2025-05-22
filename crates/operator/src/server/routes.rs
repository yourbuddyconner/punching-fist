use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Json,
};
use http::StatusCode;
use prometheus::{Encoder, TextEncoder};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use crate::{
    scheduler::TaskScheduler,
    Result,
};
use super::receivers::{AlertReceiver, PrometheusAlert};

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn metrics(
    State(scheduler): State<Arc<Mutex<TaskScheduler>>>,
) -> String {
    let scheduler = scheduler.lock().await;
    let metrics = scheduler.get_metrics();
    
    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    encoder.encode(&metrics, &mut buffer).unwrap();
    
    String::from_utf8(buffer).unwrap()
}

pub async fn alert_handler(
    State(receiver): State<Arc<dyn AlertReceiver>>,
    State(scheduler): State<Arc<Mutex<TaskScheduler>>>,
    Json(alert): Json<PrometheusAlert>,
) -> Response {
    info!("Received alert: {:?}", alert);
    
    match receiver.handle_alert(alert).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            error!("Error handling alert: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
} 