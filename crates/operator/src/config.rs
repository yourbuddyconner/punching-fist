use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::store::{DatabaseConfig, DatabaseType};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskExecutionMode {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "kubernetes")]
    Kubernetes,
}

impl Default for TaskExecutionMode {
    fn default() -> Self {
        TaskExecutionMode::Local
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    #[serde(default)]
    pub mode: TaskExecutionMode,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            mode: TaskExecutionMode::Local,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub kube: KubeConfig,
    pub openhands: OpenHandsConfig,
    #[serde(default)]
    pub execution: ExecutionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeConfig {
    pub namespace: String,
    pub service_account: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenHandsConfig {
    pub api_key: String,
    pub default_model: String,
}

impl Config {
    pub fn load() -> crate::Result<Self> {
        // Load environment variables from .env file if it exists
        let _ = dotenvy::dotenv();
        
        // Create config from environment variables with defaults
        let config = Config {
            server: ServerConfig {
                addr: std::env::var("SERVER_ADDR")
                    .unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            },
            database: DatabaseConfig {
                db_type: match std::env::var("DATABASE_TYPE")
                    .unwrap_or_else(|_| "sqlite".to_string())
                    .to_lowercase()
                    .as_str()
                {
                    "postgres" => DatabaseType::Postgres,
                    _ => DatabaseType::Sqlite,
                },
                sqlite_path: std::env::var("SQLITE_PATH")
                    .map(PathBuf::from)
                    .ok()
                    .or_else(|| Some(PathBuf::from("data/punching-fist.db"))),
                postgres_url: std::env::var("DATABASE_URL").ok(),
                max_connections: std::env::var("DATABASE_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(5),
            },
            kube: KubeConfig {
                namespace: std::env::var("KUBE_NAMESPACE")
                    .unwrap_or_else(|_| "default".to_string()),
                service_account: std::env::var("KUBE_SERVICE_ACCOUNT")
                    .unwrap_or_else(|_| "punching-fist".to_string()),
            },
            openhands: OpenHandsConfig {
                api_key: std::env::var("LLM_API_KEY")
                    .unwrap_or_else(|_| "".to_string()),
                default_model: std::env::var("LLM_MODEL")
                    .unwrap_or_else(|_| "anthropic/claude-3-5-sonnet-20241022".to_string()),
            },
            execution: ExecutionConfig {
                mode: match std::env::var("EXECUTION_MODE")
                    .unwrap_or_else(|_| "local".to_string())
                    .to_lowercase()
                    .as_str()
                {
                    "kubernetes" => TaskExecutionMode::Kubernetes,
                    _ => TaskExecutionMode::Local,
                },
            },
        };

        // Validate required fields
        if config.openhands.api_key.is_empty() {
            tracing::warn!("LLM_API_KEY is not set. OpenHands functionality may not work properly.");
        }

        // Validate database configuration
        match config.database.db_type {
            DatabaseType::Postgres => {
                if config.database.postgres_url.is_none() {
                    return Err(crate::OperatorError::Config(
                        "DATABASE_URL must be set when using PostgreSQL".to_string(),
                    ));
                }
            }
            DatabaseType::Sqlite => {
                if config.database.sqlite_path.is_none() {
                    return Err(crate::OperatorError::Config(
                        "SQLITE_PATH must be set when using SQLite".to_string(),
                    ));
                }
            }
        }

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                addr: "0.0.0.0:8080".to_string(),
            },
            database: DatabaseConfig {
                db_type: DatabaseType::Sqlite,
                sqlite_path: Some(PathBuf::from("data/punching-fist.db")),
                postgres_url: None,
                max_connections: 5,
            },
            kube: KubeConfig {
                namespace: "default".to_string(),
                service_account: "punching-fist".to_string(),
            },
            openhands: OpenHandsConfig {
                api_key: "".to_string(),
                default_model: "anthropic/claude-3-5-sonnet-20241022".to_string(),
            },
            execution: ExecutionConfig::default(),
        }
    }
} 