use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePool},
    Row,
};
use std::str::FromStr;
use uuid::Uuid;
use std::fs;
use tracing::info;

use super::{
    AlertRecord, DatabaseConfig, Store, TaskRecord, TaskResources, TaskStatus,
};
use crate::Result;
use crate::OperatorError;

pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        let path = config
            .sqlite_path
            .as_ref()
            .ok_or_else(|| OperatorError::Config("SQLite path not configured".into()))?;

        // Ensure the parent directory exists so SQLite can create the file.
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        info!("Opening SQLite database at {:?}", path);

        let options = SqliteConnectOptions::from_str(
            path
                .to_str()
                .ok_or_else(|| OperatorError::Config("Invalid SQLite path".into()))?
        )?
        .create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await?;
        Ok(Self { pool })
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn init(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await?;
        Ok(())
    }

    async fn save_alert(&self, alert: AlertRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO alerts (
                id, name, status, severity, description, labels, annotations,
                starts_at, ends_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(alert.id)
        .bind(&alert.name)
        .bind(&alert.status)
        .bind(&alert.severity)
        .bind(&alert.description)
        .bind(serde_json::to_string(&alert.labels)?)
        .bind(serde_json::to_string(&alert.annotations)?)
        .bind(alert.starts_at)
        .bind(alert.ends_at)
        .bind(alert.created_at)
        .bind(alert.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_alert(&self, id: Uuid) -> Result<Option<AlertRecord>> {
        let row = sqlx::query(
            r#"
            SELECT * FROM alerts WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(AlertRecord {
                id: row.get("id"),
                name: row.get("name"),
                status: row.get("status"),
                severity: row.get("severity"),
                description: row.get("description"),
                labels: serde_json::from_str(row.get("labels"))?,
                annotations: serde_json::from_str(row.get("annotations"))?,
                starts_at: row.get("starts_at"),
                ends_at: row.get("ends_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn save_task(&self, task: TaskRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, alert_id, prompt, model, status, max_retries, retry_count,
                timeout, resources, created_at, updated_at, started_at,
                completed_at, error
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(task.id)
        .bind(task.alert_id)
        .bind(&task.prompt)
        .bind(&task.model)
        .bind(task.status as i32)
        .bind(task.max_retries)
        .bind(task.retry_count)
        .bind(task.timeout)
        .bind(serde_json::to_string(&task.resources)?)
        .bind(task.created_at)
        .bind(task.updated_at)
        .bind(task.started_at)
        .bind(task.completed_at)
        .bind(&task.error)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_task(&self, id: Uuid) -> Result<Option<TaskRecord>> {
        let row = sqlx::query(
            r#"
            SELECT * FROM tasks WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(TaskRecord {
                id: row.get("id"),
                alert_id: row.get("alert_id"),
                prompt: row.get("prompt"),
                model: row.get("model"),
                status: match row.get::<i32, _>("status") {
                    0 => TaskStatus::Pending,
                    1 => TaskStatus::Running,
                    2 => TaskStatus::Succeeded,
                    3 => TaskStatus::Failed,
                    4 => TaskStatus::Retrying,
                    _ => return Err(OperatorError::Config("Invalid task status".into())),
                },
                max_retries: row.get("max_retries"),
                retry_count: row.get("retry_count"),
                timeout: row.get("timeout"),
                resources: serde_json::from_str(row.get("resources"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                error: row.get("error"),
            }))
        } else {
            Ok(None)
        }
    }

    async fn update_task_status(&self, id: Uuid, status: TaskStatus) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status as i32)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_task_completion(&self, id: Uuid, status: TaskStatus, started_at: Option<DateTime<Utc>>, completed_at: Option<DateTime<Utc>>, error: Option<String>) -> Result<()> {
        let now = Utc::now();
        sqlx::query(
            r#"
            UPDATE tasks
            SET status = ?, updated_at = ?, started_at = ?, completed_at = ?, error = ?
            WHERE id = ?
            "#,
        )
        .bind(status as i32)
        .bind(now)
        .bind(started_at)
        .bind(completed_at)
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_tasks(&self, limit: i64, offset: i64) -> Result<Vec<TaskRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM tasks
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            tasks.push(TaskRecord {
                id: row.get("id"),
                alert_id: row.get("alert_id"),
                prompt: row.get("prompt"),
                model: row.get("model"),
                status: match row.get::<i32, _>("status") {
                    0 => TaskStatus::Pending,
                    1 => TaskStatus::Running,
                    2 => TaskStatus::Succeeded,
                    3 => TaskStatus::Failed,
                    4 => TaskStatus::Retrying,
                    _ => return Err(OperatorError::Config("Invalid task status".into())),
                },
                max_retries: row.get("max_retries"),
                retry_count: row.get("retry_count"),
                timeout: row.get("timeout"),
                resources: serde_json::from_str(row.get("resources"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                error: row.get("error"),
            });
        }

        Ok(tasks)
    }

    async fn list_alerts(&self, limit: i64, offset: i64) -> Result<Vec<AlertRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM alerts
            ORDER BY created_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut alerts = Vec::with_capacity(rows.len());
        for row in rows {
            alerts.push(AlertRecord {
                id: row.get("id"),
                name: row.get("name"),
                status: row.get("status"),
                severity: row.get("severity"),
                description: row.get("description"),
                labels: serde_json::from_str(row.get("labels"))?,
                annotations: serde_json::from_str(row.get("annotations"))?,
                starts_at: row.get("starts_at"),
                ends_at: row.get("ends_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            });
        }

        Ok(alerts)
    }
} 