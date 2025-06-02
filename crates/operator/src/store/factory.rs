use crate::store::{DatabaseConfig, DatabaseType, SqliteStore, PostgresStore, Store};
use std::sync::Arc;

pub async fn create_store(config: &DatabaseConfig) -> crate::Result<Arc<dyn Store>> {
    match config.db_type {
        DatabaseType::Sqlite => {
            let path = config.sqlite_path
                .as_ref()
                .ok_or_else(|| crate::Error::Config("SQLite path not configured".into()))?
                .to_str()
                .unwrap_or("data/punching-fist.db");
            Ok(Arc::new(SqliteStore::new(path).await?))
        },
        DatabaseType::Postgres => {
            let connection_string = config.connection_string
                .as_ref()
                .ok_or_else(|| crate::Error::Config("PostgreSQL connection string not configured".into()))?;
            Ok(Arc::new(PostgresStore::new(connection_string).await?))
        },
    }
} 