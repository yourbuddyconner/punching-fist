use axum::{
    extract::State,
    Json,
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

pub async fn alert_handler(
    State(scheduler): State<Arc<Mutex<TaskScheduler>>>,
    Json(alert): Json<Alert>,
) -> Result<()> {
    info!("Received alert: {:?}", alert);
    
    let mut scheduler = scheduler.lock().await;
    if let Err(e) = scheduler.schedule_task(alert).await {
        error!("Error handling alert: {}", e);
        return Err(e);
    }
    
    Ok(())
} 