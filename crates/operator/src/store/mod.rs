mod config;
mod models;
mod sqlite;
mod postgres;
mod factory;

pub use config::{DatabaseConfig, DatabaseType};
pub use models::*;
pub use sqlite::SqliteStore;
pub use postgres::PostgresStore;
pub use factory::create_store;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait Store: Send + Sync {
    // Initialize database schema
    async fn init(&self) -> crate::Result<()>;
    
    // Alert operations
    async fn save_alert(&self, alert: Alert) -> crate::Result<()>;
    async fn get_alert(&self, id: Uuid) -> crate::Result<Option<Alert>>;
    async fn get_alert_by_fingerprint(&self, fingerprint: &str) -> crate::Result<Option<Alert>>;
    async fn update_alert_status(&self, id: Uuid, status: AlertStatus) -> crate::Result<()>;
    async fn update_alert_ai_analysis(&self, id: Uuid, analysis: serde_json::Value, confidence: f32) -> crate::Result<()>;
    async fn update_alert_timing(&self, id: Uuid, field: &str, timestamp: DateTime<Utc>) -> crate::Result<()>;
    async fn list_alerts(&self, limit: i64, offset: i64) -> crate::Result<Vec<Alert>>;
    async fn list_alerts_by_status(&self, status: AlertStatus, limit: i64) -> crate::Result<Vec<Alert>>;
    
    // Workflow operations
    async fn save_workflow(&self, workflow: Workflow) -> crate::Result<()>;
    async fn get_workflow(&self, id: Uuid) -> crate::Result<Option<Workflow>>;
    async fn update_workflow_status(&self, id: Uuid, status: WorkflowStatus) -> crate::Result<()>;
    async fn update_workflow_progress(&self, id: Uuid, steps_completed: i32, current_step: Option<String>) -> crate::Result<()>;
    async fn update_workflow_outputs(&self, id: Uuid, outputs: serde_json::Value) -> crate::Result<()>;
    async fn complete_workflow(&self, id: Uuid, status: WorkflowStatus, outputs: Option<serde_json::Value>, error: Option<String>) -> crate::Result<()>;
    async fn list_workflows(&self, limit: i64, offset: i64) -> crate::Result<Vec<Workflow>>;
    
    // Source event operations
    async fn save_source_event(&self, event: SourceEvent) -> crate::Result<()>;
    async fn get_source_event(&self, id: Uuid) -> crate::Result<Option<SourceEvent>>;
    async fn list_source_events(&self, source_name: &str, limit: i64) -> crate::Result<Vec<SourceEvent>>;
    
    // Workflow step operations
    async fn save_workflow_step(&self, step: WorkflowStep) -> crate::Result<()>;
    async fn get_workflow_step(&self, id: Uuid) -> crate::Result<Option<WorkflowStep>>;
    async fn update_workflow_step_status(&self, id: Uuid, status: StepStatus) -> crate::Result<()>;
    async fn complete_workflow_step(&self, id: Uuid, status: StepStatus, result: Option<serde_json::Value>, error: Option<String>) -> crate::Result<()>;
    async fn list_workflow_steps(&self, workflow_id: Uuid) -> crate::Result<Vec<WorkflowStep>>;
    
    // Sink output operations
    async fn save_sink_output(&self, output: SinkOutput) -> crate::Result<()>;
    async fn get_sink_output(&self, id: Uuid) -> crate::Result<Option<SinkOutput>>;
    async fn update_sink_output_status(&self, id: Uuid, status: SinkStatus, error: Option<String>) -> crate::Result<()>;
    async fn list_sink_outputs(&self, workflow_id: Uuid) -> crate::Result<Vec<SinkOutput>>;
    
    // Custom resource operations
    async fn save_custom_resource(&self, resource: CustomResource) -> crate::Result<()>;
    async fn get_custom_resource(&self, kind: &str, namespace: &str, name: &str) -> crate::Result<Option<CustomResource>>;
    async fn update_custom_resource_status(&self, id: Uuid, status: serde_json::Value) -> crate::Result<()>;
    async fn delete_custom_resource(&self, kind: &str, namespace: &str, name: &str) -> crate::Result<()>;
    async fn list_custom_resources(&self, kind: &str, namespace: Option<&str>) -> crate::Result<Vec<CustomResource>>;
    
    // Alert deduplication
    async fn deduplicate_alert(&self, fingerprint: &str, alert: Alert) -> crate::Result<DeduplicationResult>;
}

#[derive(Debug)]
pub enum DeduplicationResult {
    New(Alert),
    Duplicate(Alert),
    Updated(Alert),
} 