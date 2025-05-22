use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Sqlite,
    Postgres,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(alias = "type")]
    pub db_type: DatabaseType,
    pub sqlite_path: Option<PathBuf>,
    pub postgres_url: Option<String>,
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_type: DatabaseType::Sqlite,
            sqlite_path: Some(PathBuf::from("punching-fist.db")),
            postgres_url: None,
            max_connections: 5,
        }
    }
}

impl DatabaseConfig {
    pub fn sqlite(path: impl Into<PathBuf>) -> Self {
        Self {
            db_type: DatabaseType::Sqlite,
            sqlite_path: Some(path.into()),
            postgres_url: None,
            max_connections: 5,
        }
    }

    pub fn postgres(url: impl Into<String>) -> Self {
        Self {
            db_type: DatabaseType::Postgres,
            sqlite_path: None,
            postgres_url: Some(url.into()),
            max_connections: 5,
        }
    }
} 