pub mod server;
pub mod kubernetes;
pub mod openhands;
pub mod scheduler;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("Kubernetes error: {0}")]
    Kubernetes(#[from] kube::Error),
    #[error("OpenHands error: {0}")]
    OpenHands(String),
    #[error("Task error: {0}")]
    Task(String),
    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, OperatorError>;

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