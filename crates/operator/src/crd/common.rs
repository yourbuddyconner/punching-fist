use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use std::collections::HashMap;
// kube::CustomResource was used by the conflicting SinkSpec, remove if not used by other structs in this file.
// std::collections::HashMap was used by the conflicting SinkSpec and SinkConfig, remove if not used by other structs.

/// Event context passed between Source, Workflow, and Sink
#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct EventContext {
    /// Source that triggered the event
    pub source: SourceInfo,
    
    /// Workflow execution details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<WorkflowInfo>,
    
    /// Original event data from source
    pub data: serde_json::Value,
    
    /// Timestamp when event was received
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct SourceInfo {
    /// Name of the source
    pub name: String,
    
    /// Type of source
    #[serde(rename = "type")]
    pub source_type: String,
    
    /// Source namespace
    pub namespace: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct WorkflowInfo {
    /// Name of the workflow
    pub name: String,
    
    /// Workflow namespace
    pub namespace: String,
    
    /// Workflow outputs
    pub outputs: HashMap<String, String>,
    
    /// Workflow execution duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    
    /// Workflow completion time
    #[serde(rename = "completedAt", skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

// Removed conflicting SinkSpec, SinkType, and SinkConfig definitions
// The authoritative definitions are in crates/operator/src/crd/sink.rs

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SinkStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_triggered: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_count: Option<u32>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SinkConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub sink_type: String,
    pub config: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<String>>,
} 