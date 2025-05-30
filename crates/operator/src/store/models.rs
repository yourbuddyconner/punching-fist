use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use uuid::Uuid;

// Alert lifecycle tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: Uuid,
    pub external_id: Option<String>,
    pub fingerprint: String,
    pub status: AlertStatus,
    pub severity: AlertSeverity,
    pub alert_name: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub source_id: Option<Uuid>,
    pub workflow_id: Option<Uuid>,
    
    // AI Analysis
    pub ai_analysis: Option<JsonValue>,
    pub ai_confidence: Option<f32>,
    pub auto_resolved: bool,
    
    // Timing
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub received_at: DateTime<Utc>,
    pub triage_started_at: Option<DateTime<Utc>>,
    pub triage_completed_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertStatus {
    Received,
    Triaging,
    Resolved,
    Escalated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

// Workflow execution tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: Uuid,
    pub name: String,
    pub namespace: String,
    pub trigger_source: Option<String>,
    pub status: WorkflowStatus,
    
    // Execution details
    pub steps_completed: i32,
    pub total_steps: i32,
    pub current_step: Option<String>,
    
    // Context and results
    pub input_context: Option<JsonValue>,
    pub outputs: Option<JsonValue>,
    pub error: Option<String>,
    
    // Timing
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

// Source event tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEvent {
    pub id: Uuid,
    pub source_name: String,
    pub source_type: SourceType,
    pub event_data: JsonValue,
    pub workflow_triggered: Option<String>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Webhook,
    Chat,
    Schedule,
    Api,
    Kubernetes,
}

// Workflow step tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub name: String,
    pub step_type: StepType,
    pub status: StepStatus,
    
    // Step configuration
    pub config: Option<JsonValue>,
    
    // Execution details
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<JsonValue>,
    pub error: Option<String>,
    
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    Cli,
    Agent,
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

// Sink output tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SinkOutput {
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub sink_name: String,
    pub sink_type: SinkType,
    
    // Output details
    pub payload: Option<JsonValue>,
    pub status: SinkStatus,
    pub error: Option<String>,
    
    pub sent_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SinkType {
    Slack,
    AlertManager,
    Prometheus,
    Jira,
    PagerDuty,
    Workflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SinkStatus {
    Pending,
    Sent,
    Failed,
}

// Custom resource storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomResource {
    pub id: Uuid,
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub namespace: String,
    pub spec: JsonValue,
    pub status: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Helper functions for alert fingerprinting
impl Alert {
    pub fn generate_fingerprint(alert_name: &str, labels: &HashMap<String, String>) -> String {
        use std::collections::BTreeMap;
        
        // Sort labels for consistent fingerprinting
        let sorted_labels: BTreeMap<_, _> = labels.iter().collect();
        let labels_str = serde_json::to_string(&sorted_labels).unwrap_or_default();
        
        // Generate SHA256 hash
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(alert_name.as_bytes());
        hasher.update(b"-");
        hasher.update(labels_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }
} 