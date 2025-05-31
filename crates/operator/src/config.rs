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
    pub agent: AgentConfig,
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
pub struct AgentConfig {
    pub provider: String,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

impl Config {
    pub fn load() -> crate::Result<Self> {
        // Load environment variables from .env file if it exists
        let _ = dotenvy::dotenv();
        
        // Determine which LLM provider to use based on available API keys
        let (provider, has_api_key) = if std::env::var("ANTHROPIC_API_KEY").is_ok() {
            ("anthropic".to_string(), true)
        } else if std::env::var("OPENAI_API_KEY").is_ok() {
            ("openai".to_string(), true)
        } else {
            ("mock".to_string(), false)
        };
        
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
                connection_string: std::env::var("DATABASE_URL").ok(),
            },
            kube: KubeConfig {
                namespace: std::env::var("KUBE_NAMESPACE")
                    .unwrap_or_else(|_| "default".to_string()),
                service_account: std::env::var("KUBE_SERVICE_ACCOUNT")
                    .unwrap_or_else(|_| "punching-fist".to_string()),
            },
            agent: AgentConfig {
                provider: std::env::var("LLM_PROVIDER")
                    .unwrap_or_else(|_| provider),
                model: std::env::var("LLM_MODEL")
                    .unwrap_or_else(|_| match std::env::var("LLM_PROVIDER").as_deref() {
                        Ok("openai") => "gpt-4".to_string(),
                        _ => "claude-3-5-sonnet".to_string(),
                    }),
                temperature: std::env::var("LLM_TEMPERATURE")
                    .ok()
                    .and_then(|v| v.parse().ok()),
                max_tokens: std::env::var("LLM_MAX_TOKENS")
                    .ok()
                    .and_then(|v| v.parse().ok()),
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
        if !has_api_key && config.agent.provider != "mock" {
            tracing::warn!("No LLM API key found (ANTHROPIC_API_KEY or OPENAI_API_KEY). Using mock provider for testing.");
        }

        // Validate database configuration
        match config.database.db_type {
            DatabaseType::Postgres => {
                if config.database.connection_string.is_none() {
                    return Err(crate::Error::Config(
                        "DATABASE_URL must be set when using PostgreSQL".to_string(),
                    ));
                }
            }
            DatabaseType::Sqlite => {
                if config.database.sqlite_path.is_none() {
                    return Err(crate::Error::Config(
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
                connection_string: None,
            },
            kube: KubeConfig {
                namespace: "default".to_string(),
                service_account: "punching-fist".to_string(),
            },
            agent: AgentConfig {
                provider: "mock".to_string(),
                model: "claude-3-5-sonnet".to_string(),
                temperature: Some(0.7),
                max_tokens: Some(4096),
            },
            execution: ExecutionConfig::default(),
        }
    }
} 