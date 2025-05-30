use super::{DatabaseConfig, DatabaseType, Store};
use crate::Result;
use std::sync::Arc;

pub async fn create_store(config: &DatabaseConfig) -> Result<Arc<dyn Store>> {
    match config.db_type {
        DatabaseType::Sqlite => {
            let database_url = format!("sqlite:{}", config.sqlite_path.as_ref()
                .ok_or_else(|| crate::OperatorError::Config("SQLite path not configured".into()))?
                .display());
            let store = super::SqliteStore::new(&database_url).await?;
            Ok(Arc::new(store))
        },
        DatabaseType::Postgres => {
            let connection_string = config.connection_string.as_ref()
                .ok_or_else(|| crate::OperatorError::Config("PostgreSQL connection string not configured".into()))?;
            let store = super::PostgresStore::new(connection_string).await?;
            Ok(Arc::new(store))
        },
    }
} 