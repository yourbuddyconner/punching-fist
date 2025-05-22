use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use crate::{Result, OperatorError};

use super::traits::{Alert, AlertReceiver, Task, TaskResources};

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusAlert {
    pub version: String,
    pub group_key: String,
    pub truncated_alerts: Option<i32>,
    pub status: String,
    pub receiver: String,
    pub group_labels: HashMap<String, String>,
    pub common_labels: HashMap<String, String>,
    pub common_annotations: HashMap<String, String>,
    pub external_url: String,
    pub alerts: Vec<PrometheusAlertDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusAlertDetail {
    pub status: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub generator_url: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct PrometheusConfig {
    pub send_resolved: bool,
    pub max_alerts: i32,
    pub timeout: std::time::Duration,
}

impl Default for PrometheusConfig {
    fn default() -> Self {
        Self {
            send_resolved: true,
            max_alerts: 0,
            timeout: std::time::Duration::from_secs(0),
        }
    }
}

pub struct PrometheusReceiver {
    config: PrometheusConfig,
}

impl PrometheusReceiver {
    pub fn new(config: PrometheusConfig) -> Self {
        Self { config }
    }

    fn validate_prometheus_alert(&self, alert: &PrometheusAlert) -> Result<()> {
        if alert.version != "4" {
            return Err(OperatorError::Config("Unsupported alert version".into()));
        }
        Ok(())
    }

    fn transform_prometheus_alert(&self, alert: PrometheusAlert) -> Result<Task> {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            prompt: format!(
                "Handle the following Kubernetes alert:\n\
                Group: {}\n\
                Status: {}\n\
                Labels: {:?}\n\
                Annotations: {:?}",
                alert.group_key,
                alert.status,
                alert.common_labels,
                alert.common_annotations
            ),
            model: None,
            max_retries: Some(3),
            timeout: Some(300),
            resources: TaskResources {
                cpu_limit: "500m".to_string(),
                memory_limit: "512Mi".to_string(),
                cpu_request: "100m".to_string(),
                memory_request: "128Mi".to_string(),
            },
        };
        Ok(task)
    }
}

#[async_trait]
impl AlertReceiver for PrometheusReceiver {
    async fn handle_alert(&self, alert: Alert) -> Result<()> {
        self.validate_alert(&alert)?;
        let task = self.transform_alert(alert)?;
        // Schedule the task
        Ok(())
    }

    fn validate_alert(&self, alert: &Alert) -> Result<()> {
        // Basic validation
        if alert.name.is_empty() {
            return Err(OperatorError::Config("Alert name is required".into()));
        }
        if alert.status.is_empty() {
            return Err(OperatorError::Config("Alert status is required".into()));
        }
        Ok(())
    }

    fn transform_alert(&self, alert: Alert) -> Result<Task> {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            prompt: format!(
                "Handle the following Kubernetes alert:\n\
                Name: {}\n\
                Status: {}\n\
                Severity: {}\n\
                Description: {}\n\
                Labels: {:?}\n\
                Annotations: {:?}",
                alert.name,
                alert.status,
                alert.severity,
                alert.description,
                alert.labels,
                alert.annotations
            ),
            model: None,
            max_retries: Some(3),
            timeout: Some(300),
            resources: TaskResources {
                cpu_limit: "500m".to_string(),
                memory_limit: "512Mi".to_string(),
                cpu_request: "100m".to_string(),
                memory_request: "128Mi".to_string(),
            },
        };
        Ok(task)
    }
} 