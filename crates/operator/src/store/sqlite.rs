use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{sqlite::SqlitePool, Pool, Sqlite, Row};
use tracing::{debug, error, info};
use uuid::Uuid;
use std::collections::HashMap;
use serde_json::Value as JsonValue;

use crate::{
    store::{
        Alert, AlertStatus, AlertSeverity, CustomResource, DeduplicationResult,
        SinkOutput, SinkStatus, SinkType, SourceEvent, SourceType, StepStatus, StepType,
        Store, Workflow, WorkflowStatus, WorkflowStep,
    },
    Error, Result,
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
                Error::Sqlx(e)
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
                Error::Migrate(e)
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
                id, external_id, fingerprint, status, severity, alert_name, name,
                summary, description, labels, annotations, source_id, workflow_id,
                ai_analysis, ai_confidence, auto_resolved,
                starts_at, ends_at, received_at, triage_started_at,
                triage_completed_at, resolved_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
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
            _ => return Err(Error::Config(format!("Invalid timing field: {}", field))),
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
    
    // Workflow operations
    async fn save_workflow(&self, workflow: Workflow) -> Result<()> {
        debug!("Saving workflow: {}", workflow.id);
        
        let input_context_json = workflow.input_context.as_ref()
            .map(|c| serde_json::to_string(c))
            .transpose()?;
        let outputs_json = workflow.outputs.as_ref()
            .map(|o| serde_json::to_string(o))
            .transpose()?;
        
        sqlx::query(
            r#"
            INSERT INTO workflows (
                id, name, namespace, trigger_source, status,
                steps_completed, total_steps, current_step,
                input_context, outputs, error,
                started_at, completed_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                steps_completed = excluded.steps_completed,
                current_step = excluded.current_step,
                outputs = excluded.outputs,
                error = excluded.error,
                completed_at = excluded.completed_at
            "#,
        )
        .bind(workflow.id.to_string())
        .bind(&workflow.name)
        .bind(&workflow.namespace)
        .bind(&workflow.trigger_source)
        .bind(workflow.status.to_string())
        .bind(workflow.steps_completed)
        .bind(workflow.total_steps)
        .bind(&workflow.current_step)
        .bind(input_context_json)
        .bind(outputs_json)
        .bind(&workflow.error)
        .bind(workflow.started_at)
        .bind(workflow.completed_at)
        .bind(workflow.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_workflow(&self, id: Uuid) -> Result<Option<Workflow>> {
        debug!("Getting workflow: {}", id);
        
        let row = sqlx::query(
            r#"
            SELECT id, name, namespace, trigger_source, status,
                   steps_completed, total_steps, current_step,
                   input_context, outputs, error,
                   started_at, completed_at, created_at
            FROM workflows
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(r) => {
                let input_context: Option<JsonValue> = r.get::<Option<String>, _>("input_context")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                let outputs: Option<JsonValue> = r.get::<Option<String>, _>("outputs")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                
                Ok(Some(Workflow {
                    id: r.get::<String, _>("id").parse()?,
                    name: r.get("name"),
                    namespace: r.get("namespace"),
                    trigger_source: r.get("trigger_source"),
                    status: r.get::<String, _>("status").parse()?,
                    steps_completed: r.get("steps_completed"),
                    total_steps: r.get("total_steps"),
                    current_step: r.get("current_step"),
                    input_context,
                    outputs,
                    error: r.get("error"),
                    started_at: r.get("started_at"),
                    completed_at: r.get("completed_at"),
                    created_at: r.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }
    
    async fn update_workflow_status(&self, id: Uuid, status: WorkflowStatus) -> Result<()> {
        debug!("Updating workflow status: {} -> {:?}", id, status);
        
        sqlx::query(
            "UPDATE workflows SET status = ?1 WHERE id = ?2",
        )
        .bind(status.to_string())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn update_workflow_progress(&self, id: Uuid, steps_completed: i32, current_step: Option<String>) -> Result<()> {
        debug!("Updating workflow progress: {} -> step {}/{}", id, steps_completed, current_step.as_deref().unwrap_or("none"));
        
        sqlx::query(
            "UPDATE workflows SET steps_completed = ?1, current_step = ?2 WHERE id = ?3",
        )
        .bind(steps_completed)
        .bind(current_step)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn update_workflow_outputs(&self, id: Uuid, outputs: JsonValue) -> Result<()> {
        debug!("Updating workflow outputs: {}", id);
        
        let outputs_json = serde_json::to_string(&outputs)?;
        
        sqlx::query(
            "UPDATE workflows SET outputs = ?1 WHERE id = ?2",
        )
        .bind(outputs_json)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn complete_workflow(&self, id: Uuid, status: WorkflowStatus, outputs: Option<JsonValue>, error: Option<String>) -> Result<()> {
        debug!("Completing workflow: {} with status {:?}", id, status);
        
        let outputs_json = outputs.as_ref()
            .map(|o| serde_json::to_string(o))
            .transpose()?;
        
        sqlx::query(
            "UPDATE workflows SET status = ?1, outputs = ?2, error = ?3, completed_at = ?4 WHERE id = ?5",
        )
        .bind(status.to_string())
        .bind(outputs_json)
        .bind(error)
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn list_workflows(&self, limit: i64, offset: i64) -> Result<Vec<Workflow>> {
        debug!("Listing workflows: limit={}, offset={}", limit, offset);
        
        let mut workflows = Vec::new();
        let rows = sqlx::query(
            "SELECT id FROM workflows ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        
        for row in rows {
            if let Some(workflow) = self.get_workflow(row.get::<String, _>("id").parse()?).await? {
                workflows.push(workflow);
            }
        }
        
        Ok(workflows)
    }
    
    async fn save_source_event(&self, event: SourceEvent) -> Result<()> {
        debug!("Saving source event: {}", event.id);
        
        let event_data_json = serde_json::to_string(&event.event_data)?;
        
        sqlx::query(
            r#"
            INSERT INTO source_events (
                id, source_name, source_type, event_data, workflow_triggered, received_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(event.id.to_string())
        .bind(&event.source_name)
        .bind(event.source_type.to_string())
        .bind(event_data_json)
        .bind(&event.workflow_triggered)
        .bind(event.received_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_source_event(&self, id: Uuid) -> Result<Option<SourceEvent>> {
        debug!("Getting source event: {}", id);
        
        let row = sqlx::query(
            r#"
            SELECT id, source_name, source_type, event_data, workflow_triggered, received_at
            FROM source_events
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(r) => {
                let event_data: JsonValue = serde_json::from_str(r.get("event_data"))?;
                
                Ok(Some(SourceEvent {
                    id: r.get::<String, _>("id").parse()?,
                    source_name: r.get("source_name"),
                    source_type: r.get::<String, _>("source_type").parse()?,
                    event_data,
                    workflow_triggered: r.get("workflow_triggered"),
                    received_at: r.get("received_at"),
                }))
            }
            None => Ok(None),
        }
    }
    
    async fn list_source_events(&self, source_name: &str, limit: i64) -> Result<Vec<SourceEvent>> {
        debug!("Listing source events for source: {}, limit={}", source_name, limit);
        
        let mut events = Vec::new();
        let rows = sqlx::query(
            "SELECT id FROM source_events WHERE source_name = ?1 ORDER BY received_at DESC LIMIT ?2",
        )
        .bind(source_name)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        
        for row in rows {
            if let Some(event) = self.get_source_event(row.get::<String, _>("id").parse()?).await? {
                events.push(event);
            }
        }
        
        Ok(events)
    }
    
    async fn save_workflow_step(&self, step: WorkflowStep) -> Result<()> {
        debug!("Saving workflow step: {}", step.id);
        
        let config_json = step.config.as_ref()
            .map(|c| serde_json::to_string(c))
            .transpose()?;
        let result_json = step.result.as_ref()
            .map(|r| serde_json::to_string(r))
            .transpose()?;
        
        sqlx::query(
            r#"
            INSERT INTO workflow_steps (
                id, workflow_id, name, step_type, status,
                config, started_at, completed_at, result, error, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at,
                result = excluded.result,
                error = excluded.error
            "#,
        )
        .bind(step.id.to_string())
        .bind(step.workflow_id.to_string())
        .bind(&step.name)
        .bind(step.step_type.to_string())
        .bind(step.status.to_string())
        .bind(config_json)
        .bind(step.started_at)
        .bind(step.completed_at)
        .bind(result_json)
        .bind(&step.error)
        .bind(step.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_workflow_step(&self, id: Uuid) -> Result<Option<WorkflowStep>> {
        debug!("Getting workflow step: {}", id);
        
        let row = sqlx::query(
            r#"
            SELECT id, workflow_id, name, step_type, status,
                   config, started_at, completed_at, result, error, created_at
            FROM workflow_steps
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(r) => {
                let config: Option<JsonValue> = r.get::<Option<String>, _>("config")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                let result: Option<JsonValue> = r.get::<Option<String>, _>("result")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                
                Ok(Some(WorkflowStep {
                    id: r.get::<String, _>("id").parse()?,
                    workflow_id: r.get::<String, _>("workflow_id").parse()?,
                    name: r.get("name"),
                    step_type: r.get::<String, _>("step_type").parse()?,
                    status: r.get::<String, _>("status").parse()?,
                    config,
                    started_at: r.get("started_at"),
                    completed_at: r.get("completed_at"),
                    result,
                    error: r.get("error"),
                    created_at: r.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }
    
    async fn update_workflow_step_status(&self, id: Uuid, status: StepStatus) -> Result<()> {
        debug!("Updating workflow step status: {} -> {:?}", id, status);
        
        let now = if matches!(status, StepStatus::Running) {
            Some(Utc::now())
        } else {
            None
        };
        
        if let Some(timestamp) = now {
            sqlx::query(
                "UPDATE workflow_steps SET status = ?1, started_at = ?2 WHERE id = ?3",
            )
            .bind(status.to_string())
            .bind(timestamp)
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE workflow_steps SET status = ?1 WHERE id = ?2",
            )
            .bind(status.to_string())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        }
        
        Ok(())
    }
    
    async fn complete_workflow_step(&self, id: Uuid, status: StepStatus, result: Option<JsonValue>, error: Option<String>) -> Result<()> {
        debug!("Completing workflow step: {} with status {:?}", id, status);
        
        let result_json = result.as_ref()
            .map(|r| serde_json::to_string(r))
            .transpose()?;
        
        sqlx::query(
            "UPDATE workflow_steps SET status = ?1, result = ?2, error = ?3, completed_at = ?4 WHERE id = ?5",
        )
        .bind(status.to_string())
        .bind(result_json)
        .bind(error)
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn list_workflow_steps(&self, workflow_id: Uuid) -> Result<Vec<WorkflowStep>> {
        debug!("Listing workflow steps for workflow: {}", workflow_id);
        
        let mut steps = Vec::new();
        let rows = sqlx::query(
            "SELECT id FROM workflow_steps WHERE workflow_id = ?1 ORDER BY created_at",
        )
        .bind(workflow_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        
        for row in rows {
            if let Some(step) = self.get_workflow_step(row.get::<String, _>("id").parse()?).await? {
                steps.push(step);
            }
        }
        
        Ok(steps)
    }
    
    async fn save_sink_output(&self, output: SinkOutput) -> Result<()> {
        debug!("Saving sink output: {}", output.id);
        
        let payload_json = output.payload.as_ref()
            .map(|p| serde_json::to_string(p))
            .transpose()?;
        
        sqlx::query(
            r#"
            INSERT INTO sink_outputs (
                id, workflow_id, sink_name, sink_type,
                payload, status, error, sent_at, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                error = excluded.error,
                sent_at = excluded.sent_at
            "#,
        )
        .bind(output.id.to_string())
        .bind(output.workflow_id.to_string())
        .bind(&output.sink_name)
        .bind(output.sink_type.to_string())
        .bind(payload_json)
        .bind(output.status.to_string())
        .bind(&output.error)
        .bind(output.sent_at)
        .bind(output.created_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_sink_output(&self, id: Uuid) -> Result<Option<SinkOutput>> {
        debug!("Getting sink output: {}", id);
        
        let row = sqlx::query(
            r#"
            SELECT id, workflow_id, sink_name, sink_type,
                   payload, status, error, sent_at, created_at
            FROM sink_outputs
            WHERE id = ?1
            "#,
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(r) => {
                let payload: Option<JsonValue> = r.get::<Option<String>, _>("payload")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                
                Ok(Some(SinkOutput {
                    id: r.get::<String, _>("id").parse()?,
                    workflow_id: r.get::<String, _>("workflow_id").parse()?,
                    sink_name: r.get("sink_name"),
                    sink_type: r.get::<String, _>("sink_type").parse()?,
                    payload,
                    status: r.get::<String, _>("status").parse()?,
                    error: r.get("error"),
                    sent_at: r.get("sent_at"),
                    created_at: r.get("created_at"),
                }))
            }
            None => Ok(None),
        }
    }
    
    async fn update_sink_output_status(&self, id: Uuid, status: SinkStatus, error: Option<String>) -> Result<()> {
        debug!("Updating sink output status: {} -> {:?}", id, status);
        
        let sent_at = if matches!(status, SinkStatus::Sent) {
            Some(Utc::now())
        } else {
            None
        };
        
        sqlx::query(
            "UPDATE sink_outputs SET status = ?1, error = ?2, sent_at = ?3 WHERE id = ?4",
        )
        .bind(status.to_string())
        .bind(error)
        .bind(sent_at)
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn list_sink_outputs(&self, workflow_id: Uuid) -> Result<Vec<SinkOutput>> {
        debug!("Listing sink outputs for workflow: {}", workflow_id);
        
        let mut outputs = Vec::new();
        let rows = sqlx::query(
            "SELECT id FROM sink_outputs WHERE workflow_id = ?1 ORDER BY created_at",
        )
        .bind(workflow_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        
        for row in rows {
            if let Some(output) = self.get_sink_output(row.get::<String, _>("id").parse()?).await? {
                outputs.push(output);
            }
        }
        
        Ok(outputs)
    }
    
    async fn save_custom_resource(&self, resource: CustomResource) -> Result<()> {
        debug!("Saving custom resource: {}/{}/{}", resource.kind, resource.namespace, resource.name);
        
        let spec_json = serde_json::to_string(&resource.spec)?;
        let status_json = resource.status.as_ref()
            .map(|s| serde_json::to_string(s))
            .transpose()?;
        
        sqlx::query(
            r#"
            INSERT INTO custom_resources (
                id, api_version, kind, name, namespace,
                spec, status, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(kind, namespace, name) DO UPDATE SET
                api_version = excluded.api_version,
                spec = excluded.spec,
                status = excluded.status,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(resource.id.to_string())
        .bind(&resource.api_version)
        .bind(&resource.kind)
        .bind(&resource.name)
        .bind(&resource.namespace)
        .bind(spec_json)
        .bind(status_json)
        .bind(resource.created_at)
        .bind(resource.updated_at)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_custom_resource(&self, kind: &str, namespace: &str, name: &str) -> Result<Option<CustomResource>> {
        debug!("Getting custom resource: {}/{}/{}", kind, namespace, name);
        
        let row = sqlx::query(
            r#"
            SELECT id, api_version, kind, name, namespace,
                   spec, status, created_at, updated_at
            FROM custom_resources
            WHERE kind = ?1 AND namespace = ?2 AND name = ?3
            "#,
        )
        .bind(kind)
        .bind(namespace)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some(r) => {
                let spec: JsonValue = serde_json::from_str(r.get("spec"))?;
                let status: Option<JsonValue> = r.get::<Option<String>, _>("status")
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?;
                
                Ok(Some(CustomResource {
                    id: r.get::<String, _>("id").parse()?,
                    api_version: r.get("api_version"),
                    kind: r.get("kind"),
                    name: r.get("name"),
                    namespace: r.get("namespace"),
                    spec,
                    status,
                    created_at: r.get("created_at"),
                    updated_at: r.get("updated_at"),
                }))
            }
            None => Ok(None),
        }
    }
    
    async fn update_custom_resource_status(&self, id: Uuid, status: JsonValue) -> Result<()> {
        debug!("Updating custom resource status: {}", id);
        
        let status_json = serde_json::to_string(&status)?;
        
        sqlx::query(
            "UPDATE custom_resources SET status = ?1, updated_at = ?2 WHERE id = ?3",
        )
        .bind(status_json)
        .bind(Utc::now())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn delete_custom_resource(&self, kind: &str, namespace: &str, name: &str) -> Result<()> {
        debug!("Deleting custom resource: {}/{}/{}", kind, namespace, name);
        
        sqlx::query(
            "DELETE FROM custom_resources WHERE kind = ?1 AND namespace = ?2 AND name = ?3",
        )
        .bind(kind)
        .bind(namespace)
        .bind(name)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn list_custom_resources(&self, kind: &str, namespace: Option<&str>) -> Result<Vec<CustomResource>> {
        debug!("Listing custom resources: kind={}, namespace={:?}", kind, namespace);
        
        let mut resources = Vec::new();
        
        let rows = if let Some(ns) = namespace {
            sqlx::query(
                "SELECT kind, namespace, name FROM custom_resources WHERE kind = ?1 AND namespace = ?2 ORDER BY created_at DESC",
            )
            .bind(kind)
            .bind(ns)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT kind, namespace, name FROM custom_resources WHERE kind = ?1 ORDER BY created_at DESC",
            )
            .bind(kind)
            .fetch_all(&self.pool)
            .await?
        };
        
        for row in rows {
            let kind: String = row.get("kind");
            let namespace: String = row.get("namespace");
            let name: String = row.get("name");
            
            if let Some(resource) = self.get_custom_resource(&kind, &namespace, &name).await? {
                resources.push(resource);
            }
        }
        
        Ok(resources)
    }
}

// Helper implementations for parsing string to enums
impl std::str::FromStr for AlertStatus {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "received" => Ok(AlertStatus::Received),
            "triaging" => Ok(AlertStatus::Triaging),
            "resolved" => Ok(AlertStatus::Resolved),
            "escalated" => Ok(AlertStatus::Escalated),
            _ => Err(Error::Config(format!("Invalid alert status: {}", s))),
        }
    }
}

impl std::str::FromStr for AlertSeverity {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "critical" => Ok(AlertSeverity::Critical),
            "warning" => Ok(AlertSeverity::Warning),
            "info" => Ok(AlertSeverity::Info),
            _ => Err(Error::Config(format!("Invalid alert severity: {}", s))),
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

impl std::str::FromStr for SourceType {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "webhook" => Ok(SourceType::Webhook),
            "chat" => Ok(SourceType::Chat),
            "schedule" => Ok(SourceType::Schedule),
            "api" => Ok(SourceType::Api),
            "kubernetes" => Ok(SourceType::Kubernetes),
            _ => Err(Error::Config(format!("Invalid source type: {}", s))),
        }
    }
}

impl std::fmt::Display for SourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceType::Webhook => write!(f, "webhook"),
            SourceType::Chat => write!(f, "chat"),
            SourceType::Schedule => write!(f, "schedule"),
            SourceType::Api => write!(f, "api"),
            SourceType::Kubernetes => write!(f, "kubernetes"),
        }
    }
}

impl std::str::FromStr for WorkflowStatus {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(WorkflowStatus::Pending),
            "running" => Ok(WorkflowStatus::Running),
            "succeeded" => Ok(WorkflowStatus::Succeeded),
            "failed" => Ok(WorkflowStatus::Failed),
            _ => Err(Error::Config(format!("Invalid workflow status: {}", s))),
        }
    }
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkflowStatus::Pending => write!(f, "pending"),
            WorkflowStatus::Running => write!(f, "running"),
            WorkflowStatus::Succeeded => write!(f, "succeeded"),
            WorkflowStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for StepType {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "cli" => Ok(StepType::Cli),
            "agent" => Ok(StepType::Agent),
            "conditional" => Ok(StepType::Conditional),
            _ => Err(Error::Config(format!("Invalid step type: {}", s))),
        }
    }
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepType::Cli => write!(f, "cli"),
            StepType::Agent => write!(f, "agent"),
            StepType::Conditional => write!(f, "conditional"),
        }
    }
}

impl std::str::FromStr for StepStatus {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(StepStatus::Pending),
            "running" => Ok(StepStatus::Running),
            "succeeded" => Ok(StepStatus::Succeeded),
            "failed" => Ok(StepStatus::Failed),
            "skipped" => Ok(StepStatus::Skipped),
            _ => Err(Error::Config(format!("Invalid step status: {}", s))),
        }
    }
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepStatus::Pending => write!(f, "pending"),
            StepStatus::Running => write!(f, "running"),
            StepStatus::Succeeded => write!(f, "succeeded"),
            StepStatus::Failed => write!(f, "failed"),
            StepStatus::Skipped => write!(f, "skipped"),
        }
    }
}

impl std::str::FromStr for SinkType {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "slack" => Ok(SinkType::Slack),
            "alertmanager" => Ok(SinkType::AlertManager),
            "prometheus" => Ok(SinkType::Prometheus),
            "jira" => Ok(SinkType::Jira),
            "pagerduty" => Ok(SinkType::PagerDuty),
            "workflow" => Ok(SinkType::Workflow),
            _ => Err(Error::Config(format!("Invalid sink type: {}", s))),
        }
    }
}

impl std::fmt::Display for SinkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkType::Slack => write!(f, "slack"),
            SinkType::AlertManager => write!(f, "alertmanager"),
            SinkType::Prometheus => write!(f, "prometheus"),
            SinkType::Jira => write!(f, "jira"),
            SinkType::PagerDuty => write!(f, "pagerduty"),
            SinkType::Workflow => write!(f, "workflow"),
        }
    }
}

impl std::str::FromStr for SinkStatus {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(SinkStatus::Pending),
            "sent" => Ok(SinkStatus::Sent),
            "failed" => Ok(SinkStatus::Failed),
            _ => Err(Error::Config(format!("Invalid sink status: {}", s))),
        }
    }
}

impl std::fmt::Display for SinkStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SinkStatus::Pending => write!(f, "pending"),
            SinkStatus::Sent => write!(f, "sent"),
            SinkStatus::Failed => write!(f, "failed"),
        }
    }
} 