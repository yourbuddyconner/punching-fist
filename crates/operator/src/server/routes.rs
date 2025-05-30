use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use tracing::{info, error};

use crate::{
    server::Server,
    store::{self, Store},
    sources::webhook::AlertManagerWebhook,
    // Removed old imports that don't exist anymore
};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

pub async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

// TODO: Phase 1 - These routes need to be rewritten for the new architecture
// Temporarily returning empty responses to allow compilation

pub async fn create_alert(
    State(_server): State<Arc<Server>>,
    Json(_payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    // TODO: Implement for Phase 1
    (StatusCode::NOT_IMPLEMENTED, "Alert creation not yet implemented")
}

pub async fn get_alert(
    State(_server): State<Arc<Server>>,
    Path(_id): Path<Uuid>,
) -> impl IntoResponse {
    // TODO: Implement for Phase 1
    (StatusCode::NOT_IMPLEMENTED, "Alert retrieval not yet implemented")
}

pub async fn list_alerts(
    State(_server): State<Arc<Server>>,
    Query(_query): Query<ListQuery>,
) -> impl IntoResponse {
    // TODO: Implement for Phase 1
    (StatusCode::NOT_IMPLEMENTED, "Alert listing not yet implemented")
}

pub async fn webhook_alerts(
    State(server): State<Arc<Server>>,
    Path(path): Path<String>,
    Json(payload): Json<AlertManagerWebhook>,
) -> impl IntoResponse {
    info!("Received AlertManager webhook on path: /{}", path);
    
    // Get webhook configuration for this path
    let webhook_config = match server.webhook_handler.get_webhook_config(&path).await {
        Some(config) => config,
        None => {
            error!("No webhook configured for path: /{}", path);
            return (StatusCode::NOT_FOUND, "Webhook path not configured");
        }
    };

    // Process the webhook
    match server.webhook_handler.handle_alertmanager_webhook(&webhook_config, payload).await {
        Ok(alert_ids) => {
            info!("Successfully processed {} alerts", alert_ids.len());
            (StatusCode::OK, "Alerts processed successfully")
        }
        Err(e) => {
            error!("Failed to process webhook: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to process alerts")
        }
    }
}

pub async fn metrics() -> impl IntoResponse {
    // TODO: Implement Prometheus metrics endpoint
    "# HELP punchingfist_alerts_total Total number of alerts processed\n# TYPE punchingfist_alerts_total counter\npunchingfist_alerts_total 0\n"
}

// Old implementations commented out for reference during Phase 1 implementation
/*
use super::{Alert, PrometheusAlert, ServerState};
use crate::{
    Task, TaskMetrics, TaskPhase, TaskResources,
};
use chrono::Utc;
use prometheus::{Encoder, TextEncoder};

pub async fn alert_handler(
    State(state): State<Arc<ServerState>>,
    Json(alert): Json<Alert>,
) -> impl IntoResponse {
    tracing::info!("Received alert: {:?}", alert);
    
    match state.receiver.handle_alert(alert).await {
        Ok(_) => (StatusCode::OK, "Alert processed"),
        Err(e) => {
            tracing::error!("Failed to process alert: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to process alert")
        }
    }
}

// ... rest of old implementation
*/ 