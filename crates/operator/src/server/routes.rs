use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;
use std::collections::HashMap;

use crate::{
    store::{AlertRecord, TaskRecord, TaskStatus, self},
};
use super::{
    ServerState, 
    receivers::{Alert, PrometheusAlert},
};

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn metrics(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let scheduler = state.scheduler.lock().await;
    let metrics = scheduler.get_metrics();
    format!(
        "tasks_total {}\ntasks_running {}\ntasks_succeeded {}\ntasks_failed {}\n",
        metrics.tasks_total,
        metrics.tasks_running,
        metrics.tasks_succeeded,
        metrics.tasks_failed
    )
}

pub async fn prometheus_alert_handler(
    State(state): State<Arc<ServerState>>,
    Json(prometheus_payload): Json<PrometheusAlert>,
) -> impl IntoResponse {
    info!("Received Prometheus alert: {:?}", prometheus_payload);

    // Validate the Prometheus alert format
    if prometheus_payload.version != "4" {
        error!("Unsupported Prometheus alert version: {}", prometheus_payload.version);
        return StatusCode::BAD_REQUEST;
    }

    if prometheus_payload.alerts.is_empty() {
        error!("No alerts in Prometheus payload");
        return StatusCode::BAD_REQUEST;
    }

    // Process each alert in the group
    for prometheus_alert_detail in prometheus_payload.alerts {
        // Extract the main alert name from labels
        let alert_name = prometheus_alert_detail.labels
            .get("alertname")
            .unwrap_or(&"UnknownAlert".to_string())
            .clone();

        // Convert Prometheus alert to our internal Alert format
        let alert = Alert {
            name: alert_name.clone(),
            status: prometheus_alert_detail.status.clone(),
            severity: prometheus_alert_detail.labels
                .get("severity")
                .unwrap_or(&"unknown".to_string())
                .clone(),
            description: prometheus_alert_detail.annotations
                .get("description")
                .or_else(|| prometheus_alert_detail.annotations.get("summary"))
                .unwrap_or(&format!("Alert: {}", alert_name))
                .clone(),
            labels: prometheus_alert_detail.labels.clone(),
            annotations: prometheus_alert_detail.annotations.clone(),
            starts_at: prometheus_alert_detail.starts_at,
            ends_at: prometheus_alert_detail.ends_at,
        };

        // Transform the alert using the receiver
        let receiver_task = match state.receiver.transform_alert(alert.clone()) {
            Ok(task) => task,
            Err(e) => {
                error!("Failed to transform alert: {}", e);
                return StatusCode::BAD_REQUEST;
            }
        };

        // Save the alert
        let alert_record = AlertRecord {
            id: Uuid::new_v4(),
            name: alert_name.clone(),
            status: prometheus_alert_detail.status.clone(),
            severity: prometheus_alert_detail.labels
                .get("severity")
                .unwrap_or(&"unknown".to_string())
                .clone(),
            description: Some(prometheus_alert_detail.annotations
                .get("description")
                .or_else(|| prometheus_alert_detail.annotations.get("summary"))
                .unwrap_or(&format!("Alert: {}", alert_name))
                .clone()),
            labels: prometheus_alert_detail.labels.clone(),
            annotations: prometheus_alert_detail.annotations.clone(),
            starts_at: prometheus_alert_detail.starts_at,
            ends_at: prometheus_alert_detail.ends_at,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        if let Err(e) = state.store.save_alert(alert_record.clone()).await {
            error!("Failed to save alert: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        // Convert receiver TaskResources to store TaskResources
        let store_resources = store::TaskResources {
            cpu_limit: receiver_task.resources.cpu_limit.clone(),
            memory_limit: receiver_task.resources.memory_limit.clone(),
            cpu_request: receiver_task.resources.cpu_request.clone(),
            memory_request: receiver_task.resources.memory_request.clone(),
        };

        // Create and save the task
        let task_record = TaskRecord {
            id: Uuid::new_v4(),
            alert_id: alert_record.id,
            prompt: receiver_task.prompt.clone(),
            model: receiver_task.model.clone().unwrap_or_default(),
            status: TaskStatus::Pending,
            max_retries: receiver_task.max_retries.unwrap_or(3),
            retry_count: 0,
            timeout: receiver_task.timeout.unwrap_or(300),
            resources: store_resources,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
        };

        if let Err(e) = state.store.save_task(task_record.clone()).await {
            error!("Failed to save task: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }

        // Use the scheduler to handle task execution according to the configured mode
        let mut scheduler = state.scheduler.lock().await;
        if let Err(e) = scheduler.schedule_task(alert, task_record.clone()).await {
            error!("Failed to schedule task for alert {}: {}", alert_name, e);
            // Don't return error here, continue processing other alerts
            continue;
        }
        info!("Successfully scheduled task for alert: {}", alert_name);
    }

    StatusCode::OK
}

pub async fn alert_handler(
    State(state): State<Arc<ServerState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    info!("Received alert: {:?}", payload);

    // Deserialize the JSON payload into an Alert type
    let alert = match serde_json::from_value::<Alert>(payload) {
        Ok(alert) => alert,
        Err(e) => {
            error!("Failed to parse alert: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    // First create a task from the alert
    let receiver_task = match state.receiver.transform_alert(alert.clone()) {
        Ok(task) => task,
        Err(e) => {
            error!("Failed to transform alert: {}", e);
            return StatusCode::BAD_REQUEST;
        }
    };

    // Save the alert
    let alert_record = AlertRecord {
        id: Uuid::new_v4(),
        name: receiver_task.prompt.clone(),
        status: "firing".to_string(),
        severity: "critical".to_string(),
        description: None,
        labels: HashMap::new(),  // We don't have labels at this point
        annotations: HashMap::new(), // We don't have annotations at this point
        starts_at: Utc::now(),
        ends_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    if let Err(e) = state.store.save_alert(alert_record.clone()).await {
        error!("Failed to save alert: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Convert receiver TaskResources to store TaskResources
    let store_resources = store::TaskResources {
        cpu_limit: receiver_task.resources.cpu_limit.clone(),
        memory_limit: receiver_task.resources.memory_limit.clone(),
        cpu_request: receiver_task.resources.cpu_request.clone(),
        memory_request: receiver_task.resources.memory_request.clone(),
    };

    // Create and save the task
    let task_record = TaskRecord {
        id: Uuid::new_v4(),
        alert_id: alert_record.id,
        prompt: receiver_task.prompt.clone(),
        model: receiver_task.model.clone().unwrap_or_default(),
        status: TaskStatus::Pending,
        max_retries: receiver_task.max_retries.unwrap_or(3),
        retry_count: 0,
        timeout: receiver_task.timeout.unwrap_or(300),
        resources: store_resources,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        started_at: None,
        completed_at: None,
        error: None,
    };

    if let Err(e) = state.store.save_task(task_record.clone()).await {
        error!("Failed to save task: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // Use the scheduler to handle task execution according to the configured mode
    let mut scheduler = state.scheduler.lock().await;
    if let Err(e) = scheduler.schedule_task(alert, task_record.clone()).await {
        error!("Failed to schedule task for alert {}: {}", receiver_task.prompt, e);
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    info!("Successfully scheduled task for alert: {}", receiver_task.prompt);

    StatusCode::OK
} 