use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;
use tracing::{info, error};
use chrono::Utc;

use crate::{
    server::Server,
    sources::webhook::AlertManagerWebhook,
    metrics::{gather_metrics, PROCESSED_ALERTS_TOTAL},
    store::models::{Alert, AlertStatus, AlertSeverity},
};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

#[derive(Debug, Serialize)]
pub struct RootResponse {
    service: String,
    version: String,
    ui_url: String,
    endpoints: Vec<EndpointInfo>,
}

#[derive(Debug, Serialize)]
pub struct EndpointInfo {
    path: String,
    method: String,
    description: String,
}

pub async fn root() -> impl IntoResponse {
    Json(RootResponse {
        service: "punching-fist-operator".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        ui_url: "/ui/".to_string(),
        endpoints: vec![
            EndpointInfo {
                path: "/health".to_string(),
                method: "GET".to_string(),
                description: "Health check endpoint".to_string(),
            },
            EndpointInfo {
                path: "/alerts".to_string(),
                method: "GET".to_string(),
                description: "List alerts with pagination".to_string(),
            },
            EndpointInfo {
                path: "/alerts".to_string(),
                method: "POST".to_string(),
                description: "Create a new alert".to_string(),
            },
            EndpointInfo {
                path: "/alerts/{id}".to_string(),
                method: "GET".to_string(),
                description: "Get a specific alert by ID".to_string(),
            },
            EndpointInfo {
                path: "/workflows".to_string(),
                method: "GET".to_string(),
                description: "List workflows with pagination".to_string(),
            },
            EndpointInfo {
                path: "/workflows/{id}".to_string(),
                method: "GET".to_string(),
                description: "Get a specific workflow by ID".to_string(),
            },
            EndpointInfo {
                path: "/workflows/{id}/steps".to_string(),
                method: "GET".to_string(),
                description: "List steps for a workflow".to_string(),
            },
            EndpointInfo {
                path: "/workflows/{id}/outputs".to_string(),
                method: "GET".to_string(),
                description: "List sink outputs for a workflow".to_string(),
            },
            EndpointInfo {
                path: "/source-events".to_string(),
                method: "GET".to_string(),
                description: "List source events (requires source_name query param)".to_string(),
            },
            EndpointInfo {
                path: "/webhook/{path}".to_string(),
                method: "POST".to_string(),
                description: "Webhook endpoint for AlertManager".to_string(),
            },
            EndpointInfo {
                path: "/metrics".to_string(),
                method: "GET".to_string(),
                description: "Prometheus metrics endpoint".to_string(),
            },
            EndpointInfo {
                path: "/ui".to_string(),
                method: "GET".to_string(),
                description: "Web UI Dashboard - view operator state".to_string(),
            },
        ],
    })
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

#[derive(Debug, Deserialize)]
pub struct CreateAlertPayload {
    external_id: Option<String>,
    alert_name: String,
    severity: String,
    summary: Option<String>,
    description: Option<String>,
    labels: Option<HashMap<String, String>>,
    annotations: Option<HashMap<String, String>>,
    starts_at: Option<chrono::DateTime<chrono::Utc>>,
    ends_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct CreateAlertResponse {
    id: Uuid,
    message: String,
}

pub async fn create_alert(
    State(server): State<Arc<Server>>,
    Json(payload): Json<CreateAlertPayload>,
) -> impl IntoResponse {
    info!("Received request to create alert: {:?}", payload);

    let alert_id = Uuid::new_v4();
    let now = Utc::now();
    
    // Parse severity
    let severity = match payload.severity.to_lowercase().as_str() {
        "critical" => AlertSeverity::Critical,
        "warning" => AlertSeverity::Warning,
        "info" => AlertSeverity::Info,
        _ => {
            error!("Invalid severity: {}", payload.severity);
            return (
                StatusCode::BAD_REQUEST,
                Json(CreateAlertResponse {
                    id: alert_id,
                    message: format!("Invalid severity: {}. Must be one of: critical, warning, info", payload.severity),
                }),
            ).into_response();
        }
    };
    
    let labels = payload.labels.unwrap_or_default();
    let fingerprint = Alert::generate_fingerprint(&payload.alert_name, &labels);

    let new_alert = Alert {
        id: alert_id,
        external_id: payload.external_id,
        fingerprint,
        status: AlertStatus::Received,
        severity,
        alert_name: payload.alert_name,
        summary: payload.summary,
        description: payload.description,
        labels,
        annotations: payload.annotations.unwrap_or_default(),
        source_id: None,
        workflow_id: None,
        ai_analysis: None,
        ai_confidence: None,
        auto_resolved: false,
        starts_at: payload.starts_at.unwrap_or(now),
        ends_at: payload.ends_at,
        received_at: now,
        triage_started_at: None,
        triage_completed_at: None,
        resolved_at: None,
        created_at: now,
        updated_at: now,
    };

    match server.store.save_alert(new_alert).await {
        Ok(_) => {
            info!("Successfully created alert with id: {}", alert_id);
            (
                StatusCode::CREATED,
                Json(CreateAlertResponse {
                    id: alert_id,
                    message: "Alert created successfully".to_string(),
                }),
            ).into_response()
        }
        Err(e) => {
            error!("Failed to create alert: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(CreateAlertResponse {
                    id: alert_id,
                    message: format!("Failed to create alert: {}", e),
                }),
            ).into_response()
        }
    }
}

pub async fn get_alert(
    State(server): State<Arc<Server>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Received request to get alert with id: {}", id);

    match server.store.get_alert(id).await {
        Ok(Some(alert)) => {
            info!("Found alert: {:?}", alert.id);
            (StatusCode::OK, Json(alert)).into_response()
        }
        Ok(None) => {
            info!("Alert with id {} not found", id);
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Alert not found",
                "id": id
            }))).into_response()
        }
        Err(e) => {
            error!("Failed to get alert: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to get alert: {}", e),
                "id": id
            }))).into_response()
        }
    }
}

pub async fn list_alerts(
    State(server): State<Arc<Server>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20).min(100); // Cap at 100
    let offset = query.offset.unwrap_or(0);
    
    info!("Received request to list alerts with limit: {}, offset: {}", limit, offset);

    match server.store.list_alerts(limit, offset).await {
        Ok(alerts) => {
            info!("Returning {} alerts", alerts.len());
            (StatusCode::OK, Json(alerts)).into_response()
        }
        Err(e) => {
            error!("Failed to list alerts: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to list alerts: {}", e)
            }))).into_response()
        }
    }
}

pub async fn webhook_alerts(
    State(server): State<Arc<Server>>,
    Path(path): Path<String>,
    Json(payload): Json<AlertManagerWebhook>,
) -> impl IntoResponse {
    info!("Received AlertManager webhook on path: /{}", path);
    PROCESSED_ALERTS_TOTAL.inc();

    // Reconstruct the full path that was used during registration
    let full_path = format!("/webhook/{}", path);
    
    // Get webhook configuration for this path
    let webhook_config = match server.webhook_handler.get_webhook_config(&full_path).await {
        Some(config) => config,
        None => {
            error!("No webhook configured for path: {}", full_path);
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
    gather_metrics()
}

// Workflow endpoints
pub async fn list_workflows(
    State(server): State<Arc<Server>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(20).min(100);
    let offset = query.offset.unwrap_or(0);
    
    info!("Listing workflows with limit: {}, offset: {}", limit, offset);

    match server.store.list_workflows(limit, offset).await {
        Ok(workflows) => {
            info!("Returning {} workflows", workflows.len());
            (StatusCode::OK, Json(workflows)).into_response()
        }
        Err(e) => {
            error!("Failed to list workflows: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to list workflows: {}", e)
            }))).into_response()
        }
    }
}

pub async fn get_workflow(
    State(server): State<Arc<Server>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Getting workflow with id: {}", id);

    match server.store.get_workflow(id).await {
        Ok(Some(workflow)) => {
            info!("Found workflow: {:?}", workflow.id);
            (StatusCode::OK, Json(workflow)).into_response()
        }
        Ok(None) => {
            info!("Workflow with id {} not found", id);
            (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Workflow not found",
                "id": id
            }))).into_response()
        }
        Err(e) => {
            error!("Failed to get workflow: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to get workflow: {}", e),
                "id": id
            }))).into_response()
        }
    }
}

pub async fn list_workflow_steps(
    State(server): State<Arc<Server>>,
    Path(workflow_id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Listing steps for workflow: {}", workflow_id);

    match server.store.list_workflow_steps(workflow_id).await {
        Ok(steps) => {
            info!("Returning {} steps for workflow {}", steps.len(), workflow_id);
            (StatusCode::OK, Json(steps)).into_response()
        }
        Err(e) => {
            error!("Failed to list workflow steps: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to list workflow steps: {}", e),
                "workflow_id": workflow_id
            }))).into_response()
        }
    }
}

pub async fn list_workflow_outputs(
    State(server): State<Arc<Server>>,
    Path(workflow_id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Listing sink outputs for workflow: {}", workflow_id);

    match server.store.list_sink_outputs(workflow_id).await {
        Ok(outputs) => {
            info!("Returning {} outputs for workflow {}", outputs.len(), workflow_id);
            (StatusCode::OK, Json(outputs)).into_response()
        }
        Err(e) => {
            error!("Failed to list workflow outputs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to list workflow outputs: {}", e),
                "workflow_id": workflow_id
            }))).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SourceEventQuery {
    source_name: String,
    limit: Option<i64>,
}

pub async fn list_source_events(
    State(server): State<Arc<Server>>,
    Query(query): Query<SourceEventQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(50).min(100);
    
    info!("Listing source events for source: {} with limit: {}", query.source_name, limit);

    match server.store.list_source_events(&query.source_name, limit).await {
        Ok(events) => {
            info!("Returning {} events for source {}", events.len(), query.source_name);
            (StatusCode::OK, Json(events)).into_response()
        }
        Err(e) => {
            error!("Failed to list source events: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Failed to list source events: {}", e),
                "source_name": query.source_name
            }))).into_response()
        }
    }
}