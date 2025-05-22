use std::sync::Arc;
use uuid::Uuid;
use crate::{
    kubernetes::KubeClient,
    openhands::OpenHandsClient,
    server::websocket::Alert,
    Task,
    TaskMetrics,
    Result,
};

pub struct TaskScheduler {
    kube_client: Arc<KubeClient>,
    openhands_client: Arc<OpenHandsClient>,
    metrics: TaskMetrics,
}

impl TaskScheduler {
    pub fn new(
        kube_client: Arc<KubeClient>,
        openhands_client: Arc<OpenHandsClient>,
    ) -> Self {
        Self {
            kube_client,
            openhands_client,
            metrics: TaskMetrics::default(),
        }
    }

    pub async fn schedule_task(&mut self, alert: Alert) -> Result<()> {
        let task = Task {
            id: Uuid::new_v4().to_string(),
            prompt: format!(
                "Handle the following Kubernetes alert: {}\nDescription: {}\nSeverity: {}\nLabels: {:?}",
                alert.name,
                alert.description,
                alert.severity,
                alert.labels
            ),
            model: None,
            max_retries: Some(3),
            timeout: Some(300),
            resources: crate::TaskResources {
                cpu_limit: "500m".to_string(),
                memory_limit: "512Mi".to_string(),
                cpu_request: "100m".to_string(),
                memory_request: "128Mi".to_string(),
            },
        };

        self.metrics.tasks_total += 1;
        self.metrics.tasks_running += 1;

        self.kube_client.create_task_job(&task).await?;

        Ok(())
    }

    pub fn get_metrics(&self) -> TaskMetrics {
        self.metrics.clone()
    }
} 