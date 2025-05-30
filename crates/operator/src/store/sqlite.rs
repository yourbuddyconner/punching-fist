use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::{Pool, Sqlite, SqlitePool, Row};
use std::collections::HashMap;
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::{
    store::{
        Alert, AlertSeverity, AlertStatus, CustomResource, DeduplicationResult, 
        SinkOutput, SinkStatus, SinkType, SourceEvent, SourceType, StepStatus, 
        StepType, Store, Workflow, WorkflowStatus, WorkflowStep,
    },
    OperatorError, Result,
};

pub struct SqliteStore {
    pool: Pool<Sqlite>,
}

impl SqliteStore {
    pub async fn new(database_url: &str) -> Result<Self> {
        info!("Connecting to SQLite database: {}", database_url);
        
        let pool = SqlitePool::connect(database_url)
            .await
            .map_err(|e| {
                error!("Failed to connect to SQLite: {}", e);
                OperatorError::Sqlx(e)
            })?;
        
        Ok(Self { pool })
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn init(&self) -> Result<()> {
        info!("Running database migrations");
        
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                error!("Failed to run migrations: {}", e);
                OperatorError::Migrate(e)
            })?;
        
        Ok(())
    }
    
    // Alert operations
    async fn save_alert(&self, alert: Alert) -> Result<()> {
        debug!("Saving alert: {}", alert.id);
        
        let labels_json = serde_json::to_string(&alert.labels)?;
        let annotations_json = serde_json::to_string(&alert.annotations)?;
        let ai_analysis_json = alert.ai_analysis.as_ref()
            .map(|a| serde_json::to_string(a))
            .transpose()?;
        
        sqlx::query(
            r#"
            INSERT INTO alerts (
                id, external_id, fingerprint, status, severity, alert_name,
                summary, description, labels, annotations, source_id, workflow_id,
                ai_analysis, ai_confidence, auto_resolved,
                starts_at, ends_at, received_at, triage_started_at,
                triage_completed_at, resolved_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                ai_analysis = excluded.ai_analysis,
                ai_confidence = excluded.ai_confidence,
                auto_resolved = excluded.auto_resolved,
                workflow_id = excluded.workflow_id,
                triage_started_at = excluded.triage_started_at,
                triage_completed_at = excluded.triage_completed_at,
                resolved_at = excluded.resolved_at,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(alert.id.to_string())
        .bind(&alert.external_id)
        .bind(&alert.fingerprint)
        .bind(alert.status.to_string())
        .bind(alert.severity.to_string())
        .bind(&alert.alert_name)
        .bind(&alert.summary)
        .bind(&alert.description)
        .bind(labels_json)
        .bind(annotations_json)
        .bind(alert.source_id.map(|id| id.to_string()))
        .bind(alert.workflow_id.map(|id| id.to_string()))
        .bind(ai_analysis_json)
        .bind(alert.ai_confidence)
        .bind(alert.auto_resolved)
        .bind(alert.starts_at)
        .bind(alert.ends_at)
        .bind(alert.received_at)
        .bind(alert.triage_started_at)
        .bind(alert.triage_completed_at)
        .bind(alert.resolved_at)
        .bind(alert.created_at)
        .bind(alert.updated_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_alert(&self, id: Uuid) -> Result<Option<Alert>> {
        debug!("Getting alert: {}", id);
        
        let row = sqlx::query(
            r#"
            SELECT id, external_id, fingerprint, status, severity, alert_name,
                   summary, description, labels, annotations, source_id, workflow_id,
                   ai_analysis, ai_confidence, auto_resolved,
                   starts_at, ends_at, received_at, triage_started_at,
                   triage_completed_at, resolved_at, created_at, updated_at
            FROM alerts
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(r) => {
                let labels: HashMap<String, String> = serde_json::from_str(r.get("labels"))?;
                let annotations: HashMap<String, String> = serde_json::from_str(r.get("annotations"))?;
                let ai_analysis: Option<JsonValue> = r.get::<Option<String>, _>("ai_analysis")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                
                Ok(Some(Alert {
                    id: r.get::<String, _>("id").parse()?,
                    external_id: r.get("external_id"),
                    fingerprint: r.get("fingerprint"),
                    status: r.get::<String, _>("status").parse()?,
                    severity: r.get::<String, _>("severity").parse()?,
                    alert_name: r.get("alert_name"),
                    summary: r.get("summary"),
                    description: r.get("description"),
                    labels,
                    annotations,
                    source_id: r.get::<Option<String>, _>("source_id").map(|s| s.parse()).transpose()?,
                    workflow_id: r.get::<Option<String>, _>("workflow_id").map(|s| s.parse()).transpose()?,
                    ai_analysis,
                    ai_confidence: r.get::<Option<f64>, _>("ai_confidence").map(|v| v as f32),
                    auto_resolved: r.get("auto_resolved"),
                    starts_at: r.get("starts_at"),
                    ends_at: r.get("ends_at"),
                    received_at: r.get("received_at"),
                    triage_started_at: r.get("triage_started_at"),
                    triage_completed_at: r.get("triage_completed_at"),
                    resolved_at: r.get("resolved_at"),
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }
    
    async fn get_alert_by_fingerprint(&self, fingerprint: &str) -> Result<Option<Alert>> {
        debug!("Getting alert by fingerprint: {}", fingerprint);
        
        let id_row = sqlx::query(
            "SELECT id FROM alerts WHERE fingerprint = ?1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await?;
        
        match id_row {
            Some(row) => self.get_alert(row.get::<String, _>("id").parse()?).await,
            None => Ok(None),
        }
    }
    
    async fn update_alert_status(&self, id: Uuid, status: AlertStatus) -> Result<()> {
        debug!("Updating alert status: {} -> {:?}", id, status);
        
        sqlx::query(
            "UPDATE alerts SET status = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(status.to_string())
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn update_alert_ai_analysis(&self, id: Uuid, analysis: JsonValue, confidence: f32) -> Result<()> {
        debug!("Updating alert AI analysis: {}", id);
        
        let analysis_json = serde_json::to_string(&analysis)?;
        
        sqlx::query(
            "UPDATE alerts SET ai_analysis = ?1, ai_confidence = ?2, updated_at = ?3 WHERE id = ?4",
        )
        .bind(analysis_json)
        .bind(confidence as f64)
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn update_alert_timing(&self, id: Uuid, field: &str, timestamp: DateTime<Utc>) -> Result<()> {
        debug!("Updating alert timing: {} -> {}", id, field);
        
        let query = match field {
            "triage_started_at" => {
                sqlx::query(
                    "UPDATE alerts SET triage_started_at = ?1, updated_at = ?2 WHERE id = ?3",
                )
                .bind(timestamp)
                .bind(Utc::now())
                .bind(id.to_string())
            }
            "triage_completed_at" => {
                sqlx::query(
                    "UPDATE alerts SET triage_completed_at = ?1, updated_at = ?2 WHERE id = ?3",
                )
                .bind(timestamp)
                .bind(Utc::now())
                .bind(id.to_string())
            }
            "resolved_at" => {
                sqlx::query(
                    "UPDATE alerts SET resolved_at = ?1, updated_at = ?2 WHERE id = ?3",
                )
                .bind(timestamp)
                .bind(Utc::now())
                .bind(id.to_string())
            }
            _ => return Err(OperatorError::Config(format!("Invalid timing field: {}", field))),
        };
        
        query.execute(&self.pool).await?;
        Ok(())
    }
    
    async fn list_alerts(&self, limit: i64, offset: i64) -> Result<Vec<Alert>> {
        debug!("Listing alerts: limit={}, offset={}", limit, offset);
        
        let mut alerts = Vec::new();
        let rows = sqlx::query(
            "SELECT id FROM alerts ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        
        for row in rows {
            if let Some(alert) = self.get_alert(row.get::<String, _>("id").parse()?).await? {
                alerts.push(alert);
            }
        }
        
        Ok(alerts)
    }
    
    async fn list_alerts_by_status(&self, status: AlertStatus, limit: i64) -> Result<Vec<Alert>> {
        debug!("Listing alerts by status: {:?}, limit={}", status, limit);
        
        let mut alerts = Vec::new();
        let rows = sqlx::query(
            "SELECT id FROM alerts WHERE status = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(status.to_string())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        for row in rows {
            if let Some(alert) = self.get_alert(row.get::<String, _>("id").parse()?).await? {
                alerts.push(alert);
            }
        }
        
        Ok(alerts)
    }
    
    async fn deduplicate_alert(&self, fingerprint: &str, mut alert: Alert) -> Result<DeduplicationResult> {
        debug!("Deduplicating alert with fingerprint: {}", fingerprint);
        
        // Check if an alert with this fingerprint already exists
        if let Some(existing) = self.get_alert_by_fingerprint(fingerprint).await? {
            // If the existing alert is resolved, create a new one
            if existing.status == AlertStatus::Resolved {
                alert.fingerprint = fingerprint.to_string();
                self.save_alert(alert.clone()).await?;
                Ok(DeduplicationResult::New(alert))
            } else {
                // Update the existing alert's timestamp
                sqlx::query(
                    "UPDATE alerts SET updated_at = ?1 WHERE id = ?2",
                )
                .bind(Utc::now())
                .bind(existing.id.to_string())
                .execute(&self.pool)
                .await?;
                
                Ok(DeduplicationResult::Duplicate(existing))
            }
        } else {
            // New alert
            alert.fingerprint = fingerprint.to_string();
            self.save_alert(alert.clone()).await?;
            Ok(DeduplicationResult::New(alert))
        }
    }
    
    // TODO: Implement remaining methods for workflows, source events, steps, sinks, and custom resources
    
    // Placeholder implementations to satisfy the trait
    async fn save_workflow(&self, _workflow: Workflow) -> Result<()> {
        todo!("Implement save_workflow")
    }
    
    async fn get_workflow(&self, _id: Uuid) -> Result<Option<Workflow>> {
        todo!("Implement get_workflow")
    }
    
    async fn update_workflow_status(&self, _id: Uuid, _status: WorkflowStatus) -> Result<()> {
        todo!("Implement update_workflow_status")
    }
    
    async fn update_workflow_progress(&self, _id: Uuid, _steps_completed: i32, _current_step: Option<String>) -> Result<()> {
        todo!("Implement update_workflow_progress")
    }
    
    async fn update_workflow_outputs(&self, _id: Uuid, _outputs: JsonValue) -> Result<()> {
        todo!("Implement update_workflow_outputs")
    }
    
    async fn complete_workflow(&self, _id: Uuid, _status: WorkflowStatus, _outputs: Option<JsonValue>, _error: Option<String>) -> Result<()> {
        todo!("Implement complete_workflow")
    }
    
    async fn list_workflows(&self, _limit: i64, _offset: i64) -> Result<Vec<Workflow>> {
        todo!("Implement list_workflows")
    }
    
    async fn save_source_event(&self, _event: SourceEvent) -> Result<()> {
        todo!("Implement save_source_event")
    }
    
    async fn get_source_event(&self, _id: Uuid) -> Result<Option<SourceEvent>> {
        todo!("Implement get_source_event")
    }
    
    async fn list_source_events(&self, _source_name: &str, _limit: i64) -> Result<Vec<SourceEvent>> {
        todo!("Implement list_source_events")
    }
    
    async fn save_workflow_step(&self, _step: WorkflowStep) -> Result<()> {
        todo!("Implement save_workflow_step")
    }
    
    async fn get_workflow_step(&self, _id: Uuid) -> Result<Option<WorkflowStep>> {
        todo!("Implement get_workflow_step")
    }
    
    async fn update_workflow_step_status(&self, _id: Uuid, _status: StepStatus) -> Result<()> {
        todo!("Implement update_workflow_step_status")
    }
    
    async fn complete_workflow_step(&self, _id: Uuid, _status: StepStatus, _result: Option<JsonValue>, _error: Option<String>) -> Result<()> {
        todo!("Implement complete_workflow_step")
    }
    
    async fn list_workflow_steps(&self, _workflow_id: Uuid) -> Result<Vec<WorkflowStep>> {
        todo!("Implement list_workflow_steps")
    }
    
    async fn save_sink_output(&self, _output: SinkOutput) -> Result<()> {
        todo!("Implement save_sink_output")
    }
    
    async fn get_sink_output(&self, _id: Uuid) -> Result<Option<SinkOutput>> {
        todo!("Implement get_sink_output")
    }
    
    async fn update_sink_output_status(&self, _id: Uuid, _status: SinkStatus, _error: Option<String>) -> Result<()> {
        todo!("Implement update_sink_output_status")
    }
    
    async fn list_sink_outputs(&self, _workflow_id: Uuid) -> Result<Vec<SinkOutput>> {
        todo!("Implement list_sink_outputs")
    }
    
    async fn save_custom_resource(&self, _resource: CustomResource) -> Result<()> {
        todo!("Implement save_custom_resource")
    }
    
    async fn get_custom_resource(&self, _kind: &str, _namespace: &str, _name: &str) -> Result<Option<CustomResource>> {
        todo!("Implement get_custom_resource")
    }
    
    async fn update_custom_resource_status(&self, _id: Uuid, _status: JsonValue) -> Result<()> {
        todo!("Implement update_custom_resource_status")
    }
    
    async fn delete_custom_resource(&self, _kind: &str, _namespace: &str, _name: &str) -> Result<()> {
        todo!("Implement delete_custom_resource")
    }
    
    async fn list_custom_resources(&self, _kind: &str, _namespace: Option<&str>) -> Result<Vec<CustomResource>> {
        todo!("Implement list_custom_resources")
    }
}

// Helper implementations for parsing string to enums
impl std::str::FromStr for AlertStatus {
    type Err = OperatorError;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "received" => Ok(AlertStatus::Received),
            "triaging" => Ok(AlertStatus::Triaging),
            "resolved" => Ok(AlertStatus::Resolved),
            "escalated" => Ok(AlertStatus::Escalated),
            _ => Err(OperatorError::Config(format!("Invalid alert status: {}", s))),
        }
    }
}

impl std::str::FromStr for AlertSeverity {
    type Err = OperatorError;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "critical" => Ok(AlertSeverity::Critical),
            "warning" => Ok(AlertSeverity::Warning),
            "info" => Ok(AlertSeverity::Info),
            _ => Err(OperatorError::Config(format!("Invalid alert severity: {}", s))),
        }
    }
}

impl std::fmt::Display for AlertStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertStatus::Received => write!(f, "received"),
            AlertStatus::Triaging => write!(f, "triaging"),
            AlertStatus::Resolved => write!(f, "resolved"),
            AlertStatus::Escalated => write!(f, "escalated"),
        }
    }
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Critical => write!(f, "critical"),
            AlertSeverity::Warning => write!(f, "warning"),
            AlertSeverity::Info => write!(f, "info"),
        }
    }
} 