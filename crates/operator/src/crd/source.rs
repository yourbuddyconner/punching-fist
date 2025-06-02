use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(CustomResource, Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[kube(
    group = "punchingfist.io",
    version = "v1alpha1",
    kind = "Source",
    namespaced,
    status = "SourceStatus"
)]
pub struct SourceSpec {
    /// Type of source: webhook, chat, schedule, api, kubernetes
    #[serde(rename = "type")]
    pub source_type: SourceType,
    
    /// Configuration specific to the source type
    pub config: SourceConfig,
    
    /// Name of the workflow to trigger
    #[serde(rename = "triggerWorkflow")]
    pub trigger_workflow: String,
    
    /// Additional context to pass to the workflow
    #[serde(default)]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Webhook,
    Chat,
    Schedule,
    Api,
    Kubernetes,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(untagged)]
pub enum SourceConfig {
    Webhook(WebhookConfig),
    Chat(ChatConfig),
    Schedule(ScheduleConfig),
    Api(ApiConfig),
    Kubernetes(KubernetesConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct WebhookConfig {
    /// Path to expose the webhook on
    pub path: String,
    
    /// Filters to apply to incoming webhooks
    #[serde(default)]
    pub filters: HashMap<String, Vec<String>>,
    
    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<AuthConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ChatConfig {
    /// Chat platform (e.g., slack)
    pub platform: String,
    
    /// Trigger type: mention or command
    pub trigger: String,
    
    /// Channel to monitor
    pub channel: String,
    
    /// Command to listen for (if trigger is command)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ScheduleConfig {
    /// Cron expression
    pub cron: String,
    
    /// Timezone for the schedule
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct ApiConfig {
    /// API endpoint path
    pub endpoint: String,
    
    /// HTTP method
    pub method: String,
    
    /// Authentication configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authentication: Option<AuthConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct KubernetesConfig {
    /// Resource type to watch
    pub resource: String,
    
    /// Event type to watch for
    pub event: String,
    
    /// Label selector for filtering resources
    #[serde(rename = "labelSelector", skip_serializing_if = "Option::is_none")]
    pub label_selector: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct AuthConfig {
    /// Authentication type
    #[serde(rename = "type")]
    pub auth_type: String,
    
    /// Reference to secret containing credentials
    #[serde(rename = "secretRef", skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct SourceStatus {
    /// Whether the source is ready
    pub ready: bool,
    
    /// Last time an event was received
    #[serde(rename = "lastEventTime", skip_serializing_if = "Option::is_none")]
    pub last_event_time: Option<String>,
    
    /// Number of events processed
    #[serde(rename = "eventsProcessed", default)]
    pub events_processed: i64,
    
    /// Current conditions
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct Condition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    #[serde(rename = "lastTransitionTime")]
    pub last_transition_time: String,
}

fn default_timezone() -> String {
    "UTC".to_string()
} 