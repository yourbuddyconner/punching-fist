pub mod config;
pub mod controllers;
pub mod crd;
pub mod metrics;
pub mod sources;
pub mod store;
pub mod server;
// pub mod kubernetes;  // Old KubeClient - replaced with kube::Client
pub mod workflow;
pub mod agent;
pub mod sinks;
pub mod template;

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
    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
} 