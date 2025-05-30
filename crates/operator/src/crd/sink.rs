use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(CustomResource, Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[kube(
    group = "punchingfist.io",
    version = "v1alpha1",
    kind = "Sink",
    namespaced,
    status = "SinkStatus"
)]
pub struct SinkSpec {
    /// Type of sink: slack, alertmanager, prometheus, jira, pagerduty, workflow
    #[serde(rename = "type")]
    pub sink_type: SinkType,
    
    /// Sink configuration
    pub config: SinkConfig,
    
    /// Condition to evaluate before sending to sink
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SinkType {
    Slack,
    AlertManager,
    Prometheus,
    Jira,
    PagerDuty,
    Workflow,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct SinkConfig {
    /// Slack configuration
    /// Channel to send messages to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    
    /// Bot token secret reference
    #[serde(rename = "botToken", skip_serializing_if = "Option::is_none")]
    pub bot_token: Option<String>,
    
    /// Message type: message or thread (for Slack)
    #[serde(rename = "messageType", skip_serializing_if = "Option::is_none")]
    pub message_type: Option<String>,
    
    /// Users to mention (for Slack)
    #[serde(rename = "mentionUsers", default)]
    pub mention_users: Vec<String>,
    
    /// AlertManager/Prometheus endpoint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    
    /// Action to perform (for AlertManager: resolve, annotate, silence; for PagerDuty: trigger, resolve)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    
    /// Pushgateway endpoint (for Prometheus)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pushgateway: Option<String>,
    
    /// Job name for metrics (for Prometheus)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<String>,
    
    /// Metrics to push (for Prometheus)
    #[serde(default)]
    pub metrics: HashMap<String, String>,
    
    /// Project key (for JIRA)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    
    /// Issue type (for JIRA)
    #[serde(rename = "issueType", skip_serializing_if = "Option::is_none")]
    pub issue_type: Option<String>,
    
    /// Credentials secret reference (for JIRA)
    #[serde(rename = "credentialsSecret", skip_serializing_if = "Option::is_none")]
    pub credentials_secret: Option<String>,
    
    /// Routing key (for PagerDuty)
    #[serde(rename = "routingKey", skip_serializing_if = "Option::is_none")]
    pub routing_key: Option<String>,
    
    /// Name of the workflow to trigger (for Workflow sink)
    #[serde(rename = "workflowName", skip_serializing_if = "Option::is_none")]
    pub workflow_name: Option<String>,
    
    /// Condition to trigger the workflow (for Workflow sink)
    #[serde(rename = "triggerCondition", skip_serializing_if = "Option::is_none")]
    pub trigger_condition: Option<String>,
    
    /// Generic template for formatting output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    
    /// Additional context
    #[serde(default)]
    pub context: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, JsonSchema)]
pub struct SinkStatus {
    /// Whether the sink is ready
    pub ready: bool,
    
    /// Last time a message was sent
    #[serde(rename = "lastSentTime", skip_serializing_if = "Option::is_none")]
    pub last_sent_time: Option<String>,
    
    /// Number of messages sent
    #[serde(rename = "messagesSent", default)]
    pub messages_sent: i64,
    
    /// Last error
    #[serde(rename = "lastError", skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    
    /// Conditions
    #[serde(default)]
    pub conditions: Vec<super::source::Condition>,
} 