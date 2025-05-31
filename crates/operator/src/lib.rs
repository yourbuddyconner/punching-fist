pub mod config;
pub mod controllers;
pub mod crd;
pub mod sources;
pub mod store;
pub mod server;
// pub mod kubernetes;  // Old KubeClient - replaced with kube::Client
pub mod scheduler;
pub mod workflow;
pub mod agent;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Kubernetes error: {0}")]
    Kubernetes(String),
    #[error("Agent error: {0}")]
    Agent(String),
    #[error("Task error: {0}")]
    Task(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Database migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Execution error: {0}")]
    Execution(String),
}

pub type Result<T> = std::result::Result<T, Error>;

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

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub phase: TaskPhase,
    pub start_time: Option<DateTime<Utc>>,
    pub completion_time: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub retry_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Retrying,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskMetrics {
    pub tasks_total: u64,
    pub tasks_running: u64,
    pub tasks_succeeded: u64,
    pub tasks_failed: u64,
}

impl Default for TaskMetrics {
    fn default() -> Self {
        Self {
            tasks_total: 0,
            tasks_running: 0,
            tasks_succeeded: 0,
            tasks_failed: 0,
        }
    }
} 