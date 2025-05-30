use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

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
    pub outputs: std::collections::HashMap<String, String>,
    
    /// Workflow execution duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    
    /// Workflow completion time
    #[serde(rename = "completedAt", skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
} 