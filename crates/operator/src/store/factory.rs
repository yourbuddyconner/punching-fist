use crate::Result;

use super::{DatabaseConfig, PostgresStore, SqliteStore, Store};

pub async fn create_store(config: &DatabaseConfig) -> Result<Box<dyn Store>> {
    match config.db_type {
        super::DatabaseType::Sqlite => {
            let store = SqliteStore::new(config).await?;
            Ok(Box::new(store))
        }
        super::DatabaseType::Postgres => {
            let store = PostgresStore::new(config).await?;
            Ok(Box::new(store))
        }
    }
} 