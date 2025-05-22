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
    pub api_url: String,
    pub api_key: String,
}

impl Config {
    pub fn load() -> crate::Result<Self> {
        let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.yaml".to_string());
        let config = std::fs::read_to_string(config_path)?;
        let config: Config = serde_yaml::from_str(&config)?;
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
                api_url: "http://localhost:8080".to_string(),
                api_key: "".to_string(),
            },
            execution: ExecutionConfig::default(),
        }
    }
} 