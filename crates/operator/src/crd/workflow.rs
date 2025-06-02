use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(CustomResource, Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[kube(
    group = "punchingfist.io",
    version = "v1alpha1",
    kind = "Workflow",
    namespaced,
    status = "WorkflowStatus"
)]
pub struct WorkflowSpec {
    /// Runtime configuration for the workflow
    pub runtime: RuntimeConfig,
    
    /// Steps to execute in the workflow
    pub steps: Vec<Step>,
    
    /// Output definitions
    #[serde(default)]
    pub outputs: Vec<OutputDef>,
    
    /// Sinks to send results to
    pub sinks: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct RuntimeConfig {
    /// Container image to use for execution
    pub image: String,
    
    /// LLM configuration
    #[serde(rename = "llmConfig")]
    pub llm_config: LLMConfig,
    
    /// Environment variables
    #[serde(default)]
    pub environment: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct LLMConfig {
    /// LLM provider (local, claude, openai)
    pub provider: String,
    
    /// Endpoint URL for the LLM (only needed for local/custom providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    
    /// Model to use
    pub model: String,
    
    /// API key secret reference
    #[serde(rename = "apiKeySecret", skip_serializing_if = "Option::is_none")]
    pub api_key_secret: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Step {
    /// Step name
    pub name: String,
    
    /// Step type: cli, agent, conditional
    #[serde(rename = "type")]
    pub step_type: StepType,
    
    /// Command to execute (for CLI steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    
    /// Goal for agent (for agent steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    
    /// Available tools for agent
    #[serde(default)]
    pub tools: Vec<Tool>,
    
    /// Maximum iterations for agent
    #[serde(rename = "maxIterations", skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<i32>,
    
    /// Timeout in minutes
    #[serde(rename = "timeoutMinutes", skip_serializing_if = "Option::is_none")]
    pub timeout_minutes: Option<i32>,
    
    /// Whether approval is required before execution
    #[serde(rename = "approvalRequired", default)]
    pub approval_required: bool,
    
    /// Condition for conditional steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    
    /// Nested agent configuration for conditional steps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<Box<Step>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    Cli,
    Agent,
    Conditional,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum Tool {
    Named(String),
    Detailed(DetailedTool),
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct DetailedTool {
    /// Tool name
    pub name: String,
    
    /// Tool description
    pub description: String,
    
    /// Custom command (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    
    /// Endpoint for API tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct OutputDef {
    /// Output name
    pub name: String,
    
    /// Value expression
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct WorkflowStatus {
    /// Current phase: pending, running, succeeded, failed
    pub phase: String,
    
    /// Start time
    #[serde(rename = "startTime", skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    
    /// Completion time
    #[serde(rename = "completionTime", skip_serializing_if = "Option::is_none")]
    pub completion_time: Option<String>,
    
    /// Step statuses
    #[serde(default)]
    pub steps: Vec<StepStatus>,
    
    /// Output values
    #[serde(default)]
    pub outputs: HashMap<String, String>,
    
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    
    /// Conditions
    #[serde(default)]
    pub conditions: Vec<super::source::Condition>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct StepStatus {
    /// Step name
    pub name: String,
    
    /// Step phase: pending, running, succeeded, failed
    pub phase: String,
    
    /// Start time
    #[serde(rename = "startTime", skip_serializing_if = "Option::is_none")]
    pub start_time: Option<String>,
    
    /// Completion time
    #[serde(rename = "completionTime", skip_serializing_if = "Option::is_none")]
    pub completion_time: Option<String>,
    
    /// Step result
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    
    /// Error if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
} 