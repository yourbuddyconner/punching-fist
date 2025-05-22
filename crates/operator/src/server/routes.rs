use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use uuid::Uuid;
use std::collections::HashMap;

use crate::{
    scheduler::TaskScheduler,
    store::{AlertRecord, Store, TaskRecord, TaskStatus, self},
    Task, TaskResources as CoreTaskResources,
    kubernetes::KubeClient,
};
use super::{
    ServerState, 
    receivers::{Alert, TaskResources as ReceiverTaskResources},
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
    let receiver_task = match state.receiver.transform_alert(alert) {
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

    // Convert receiver Task to core Task for Kubernetes
    let core_task = Task {
        id: receiver_task.id.clone(),
        prompt: receiver_task.prompt.clone(),
        model: receiver_task.model.clone(),
        max_retries: receiver_task.max_retries,
        timeout: receiver_task.timeout,
        resources: CoreTaskResources {
            cpu_limit: receiver_task.resources.cpu_limit.clone(),
            memory_limit: receiver_task.resources.memory_limit.clone(),
            cpu_request: receiver_task.resources.cpu_request.clone(),
            memory_request: receiver_task.resources.memory_request.clone(),
        },
    };

    // Create a Kubernetes client and submit the task directly
    match KubeClient::new().await {
        Ok(kube_client) => {
            if let Err(e) = kube_client.create_task_job(&core_task).await {
                error!("Failed to create task job: {}", e);
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        },
        Err(e) => {
            error!("Failed to create Kubernetes client: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    StatusCode::OK
} 