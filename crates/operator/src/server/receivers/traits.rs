use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::Result;

#[derive(Debug, Serialize, Deserialize)]
pub struct Alert {
    pub name: String,
    pub status: String,
    pub severity: String,
    pub description: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub max_retries: Option<i32>,
    pub timeout: Option<i32>,
    pub resources: TaskResources,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResources {
    pub cpu_limit: String,
    pub memory_limit: String,
    pub cpu_request: String,
    pub memory_request: String,
}

#[async_trait]
pub trait AlertReceiver: Send + Sync {
    async fn handle_alert(&self, alert: Alert) -> Result<()>;
    fn validate_alert(&self, alert: &Alert) -> Result<()>;
    fn transform_alert(&self, alert: Alert) -> Result<Task>;
} 