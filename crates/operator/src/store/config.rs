use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    #[serde(rename = "type")]
    pub db_type: DatabaseType,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sqlite_path: Option<PathBuf>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_string: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Sqlite,
    Postgres,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            db_type: DatabaseType::Sqlite,
            sqlite_path: Some(PathBuf::from("data/punchingfist.db")),
            connection_string: None,
        }
    }
}

impl DatabaseConfig {
    pub fn validate(&self) -> Result<(), String> {
        match self.db_type {
            DatabaseType::Sqlite => {
                if self.sqlite_path.is_none() {
                    return Err("SQLite path is required for SQLite database type".to_string());
                }
            }
            DatabaseType::Postgres => {
                if self.connection_string.is_none() {
                    return Err("Connection string is required for PostgreSQL database type".to_string());
                }
            }
        }
        Ok(())
    }
} 