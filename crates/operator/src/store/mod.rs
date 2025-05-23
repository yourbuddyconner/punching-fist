mod config;
mod sqlite;
mod postgres;
mod factory;

pub use config::{DatabaseConfig, DatabaseType};
pub use sqlite::SqliteStore;
pub use postgres::PostgresStore;
pub use factory::create_store;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRecord {
    pub id: Uuid,
    pub name: String,
    pub status: String,
    pub severity: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: Uuid,
    pub alert_id: Uuid,
    pub prompt: String,
    pub model: String,
    pub status: TaskStatus,
    pub max_retries: i32,
    pub retry_count: i32,
    pub timeout: i32,
    pub resources: TaskResources,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending = 0,
    Running = 1,
    Succeeded = 2,
    Failed = 3,
    Retrying = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResources {
    pub cpu_limit: String,
    pub memory_limit: String,
    pub cpu_request: String,
    pub memory_request: String,
}

#[async_trait]
pub trait Store: Send + Sync {
    async fn init(&self) -> crate::Result<()>;
    async fn save_alert(&self, alert: AlertRecord) -> crate::Result<()>;
    async fn get_alert(&self, id: Uuid) -> crate::Result<Option<AlertRecord>>;
    async fn save_task(&self, task: TaskRecord) -> crate::Result<()>;
    async fn get_task(&self, id: Uuid) -> crate::Result<Option<TaskRecord>>;
    async fn update_task_status(&self, id: Uuid, status: TaskStatus) -> crate::Result<()>;
    async fn update_task_completion(&self, id: Uuid, status: TaskStatus, started_at: Option<DateTime<Utc>>, completed_at: Option<DateTime<Utc>>, error: Option<String>) -> crate::Result<()>;
    async fn list_tasks(&self, limit: i64, offset: i64) -> crate::Result<Vec<TaskRecord>>;
    async fn list_alerts(&self, limit: i64, offset: i64) -> crate::Result<Vec<AlertRecord>>;
} 