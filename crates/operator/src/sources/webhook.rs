use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use kube::Client;

use crate::{
    store::{
        Alert, AlertStatus, AlertSeverity, Store, SourceEvent, SourceType,
    },
    Result,
    crd::Workflow,
    workflow::WorkflowEngine,
};

#[derive(Debug, Clone)]
pub struct WebhookConfig {
    pub source_name: String,
    pub path: String,
    pub filters: HashMap<String, Vec<String>>,
    pub workflow_name: String,
    pub trigger_workflow: Option<String>,
}

pub struct WebhookHandler {
    store: Arc<dyn Store>,
    client: Client,
    webhook_configs: Arc<RwLock<HashMap<String, WebhookConfig>>>,
    workflow_engine: Option<Arc<WorkflowEngine>>,
}

// AlertManager webhook payload structures
#[derive(Debug, Deserialize, Serialize)]
pub struct AlertManagerWebhook {
    pub receiver: String,
    pub status: String,
    pub alerts: Vec<AlertManagerAlert>,
    #[serde(rename = "groupLabels")]
    pub group_labels: HashMap<String, String>,
    #[serde(rename = "commonLabels")]
    pub common_labels: HashMap<String, String>,
    #[serde(rename = "commonAnnotations")]
    pub common_annotations: HashMap<String, String>,
    #[serde(rename = "externalURL")]
    pub external_url: String,
    pub version: String,
    #[serde(rename = "groupKey")]
    pub group_key: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AlertManagerAlert {
    pub status: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    #[serde(rename = "startsAt")]
    pub starts_at: DateTime<Utc>,
    #[serde(rename = "endsAt")]
    pub ends_at: Option<DateTime<Utc>>,
    #[serde(rename = "generatorURL")]
    pub generator_url: String,
    pub fingerprint: String,
}

impl WebhookHandler {
    pub fn new(store: Arc<dyn Store>, client: Client) -> Self {
        Self {
            store,
            client,
            webhook_configs: Arc::new(RwLock::new(HashMap::new())),
            workflow_engine: None,
        }
    }

    pub fn with_workflow_engine(mut self, engine: Arc<WorkflowEngine>) -> Self {
        self.workflow_engine = Some(engine);
        self
    }

    pub async fn register_webhook(
        &self,
        source_name: &str,
        path: &str,
        filters: HashMap<String, Vec<String>>,
        workflow_name: String,
        trigger_workflow: Option<String>,
    ) -> Result<()> {
        let mut webhooks = self.webhook_configs.write().await;
        
        let config = WebhookConfig {
            source_name: source_name.to_string(),
            path: path.to_string(),
            filters,
            workflow_name,
            trigger_workflow,
        };

        info!("Registered webhook for source {} at path {}", source_name, path);
        webhooks.insert(path.to_string(), config);
        
        Ok(())
    }

    pub async fn get_webhook_config(&self, path: &str) -> Option<WebhookConfig> {
        let webhooks = self.webhook_configs.read().await;
        webhooks.get(path).cloned()
    }

    pub async fn handle_alertmanager_webhook(
        &self,
        webhook_config: &WebhookConfig,
        payload: AlertManagerWebhook,
    ) -> Result<Vec<Uuid>> {
        info!(
            "Processing AlertManager webhook for source {} with {} alerts",
            webhook_config.source_name,
            payload.alerts.len()
        );

        let mut processed_alert_ids = Vec::new();

        for alert in payload.alerts {
            // Apply filters
            if !self.should_process_alert(&alert, &webhook_config.filters) {
                info!("Alert filtered out: {:?}", alert.labels);
                continue;
            }

            // Generate fingerprint for deduplication
            let alert_name = alert.labels.get("alertname")
                .unwrap_or(&"unknown".to_string())
                .clone();
            
            let fingerprint = Alert::generate_fingerprint(&alert_name, &alert.labels);

            // Check for existing alert with same fingerprint
            let existing_alert = self.store.get_alert_by_fingerprint(&fingerprint).await?;

            let alert_id = if let Some(existing) = existing_alert {
                info!("Found existing alert with fingerprint {}", fingerprint);
                
                // Update existing alert if it was resolved
                if existing.status == AlertStatus::Resolved {
                    info!("Reopening resolved alert {}", existing.id);
                    self.store.update_alert_status(existing.id, AlertStatus::Received).await?;
                    self.store.update_alert_timing(existing.id, "starts_at", alert.starts_at).await?;
                    if let Some(ends_at) = alert.ends_at {
                        self.store.update_alert_timing(existing.id, "ends_at", ends_at).await?;
                    }
                }
                
                existing.id
            } else {
                // Create new alert
                let severity = self.determine_severity(&alert.labels);
                
                let new_alert = Alert {
                    id: Uuid::new_v4(),
                    external_id: Some(alert.fingerprint.clone()),
                    fingerprint,
                    status: AlertStatus::Received,
                    severity,
                    alert_name,
                    summary: alert.annotations.get("summary").cloned(),
                    description: alert.annotations.get("description").cloned(),
                    labels: alert.labels.clone(),
                    annotations: alert.annotations.clone(),
                    source_id: None, // TODO: link to Source CR
                    workflow_id: None,
                    ai_analysis: None,
                    ai_confidence: None,
                    auto_resolved: false,
                    starts_at: alert.starts_at,
                    ends_at: alert.ends_at,
                    received_at: Utc::now(),
                    triage_started_at: None,
                    triage_completed_at: None,
                    resolved_at: None,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                };

                self.store.save_alert(new_alert.clone()).await?;
                info!("Created new alert {} with fingerprint {}", new_alert.id, new_alert.fingerprint);
                
                new_alert.id
            };

            processed_alert_ids.push(alert_id);

            // Create source event
            let source_event = SourceEvent {
                id: Uuid::new_v4(),
                source_name: webhook_config.source_name.clone(),
                source_type: SourceType::Webhook,
                event_data: serde_json::to_value(&alert)?,
                workflow_triggered: webhook_config.trigger_workflow.clone(),
                received_at: Utc::now(),
            };

            self.store.save_source_event(source_event).await?;
            
            // TODO: Trigger workflow execution
            info!(
                "Should trigger workflow {} for alert {}",
                webhook_config.workflow_name, alert_id
            );
        }

        Ok(processed_alert_ids)
    }

    fn should_process_alert(
        &self,
        alert: &AlertManagerAlert,
        filters: &HashMap<String, Vec<String>>,
    ) -> bool {
        for (key, allowed_values) in filters {
            if let Some(alert_value) = alert.labels.get(key) {
                if !allowed_values.contains(alert_value) {
                    return false;
                }
            } else {
                // If filter key is not in alert labels, don't process
                return false;
            }
        }
        true
    }

    fn determine_severity(&self, labels: &HashMap<String, String>) -> AlertSeverity {
        if let Some(severity) = labels.get("severity") {
            match severity.to_lowercase().as_str() {
                "critical" => AlertSeverity::Critical,
                "warning" => AlertSeverity::Warning,
                "info" => AlertSeverity::Info,
                _ => AlertSeverity::Warning,
            }
        } else {
            AlertSeverity::Warning
        }
    }

    async fn trigger_workflow(&self, workflow_name: &str, alert: &Alert) -> Result<()> {
        info!("Triggering workflow {} for alert {}", workflow_name, alert.id);
        
        // Get workflow from Kubernetes
        let api: kube::Api<Workflow> = kube::Api::all(self.client.clone());
        
        let workflow = api.get(workflow_name).await
            .map_err(|e| crate::Error::Kubernetes(format!("Failed to get workflow {}: {}", workflow_name, e)))?;
        
        // Queue workflow for execution if we have an engine
        if let Some(engine) = &self.workflow_engine {
            // Create a workflow instance with alert context
            let mut workflow_instance = workflow.clone();
            
            // Add alert context to workflow metadata (will be passed to context)
            if workflow_instance.metadata.annotations.is_none() {
                workflow_instance.metadata.annotations = Some(Default::default());
            }
            workflow_instance.metadata.annotations.as_mut().unwrap().insert(
                "alert.id".to_string(),
                alert.id.to_string(),
            );
            workflow_instance.metadata.annotations.as_mut().unwrap().insert(
                "alert.name".to_string(),
                alert.alert_name.clone(),
            );
            workflow_instance.metadata.annotations.as_mut().unwrap().insert(
                "alert.severity".to_string(),
                format!("{:?}", alert.severity),
            );
            
            engine.queue_workflow(workflow_instance).await?;
            
            // Update alert with workflow ID
            self.store.update_alert_timing(alert.id, "triage_started_at", chrono::Utc::now()).await?;
        } else {
            warn!("Workflow engine not available, cannot trigger workflow");
        }
        
        Ok(())
    }
} 