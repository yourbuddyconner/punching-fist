use std::sync::Arc;
use uuid::Uuid;
use crate::{
    kubernetes::KubeClient,
    openhands::OpenHandsClient,
    server::Alert,
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

        // Always try to offload the work to Kubernetes first. If that fails
        // (for example when the operator is running outside a cluster during
        // local development), fall back to executing OpenHands in-process via
        // headless mode.

        if let Err(k8s_err) = self.kube_client.create_task_job(&task).await {
            tracing::warn!(error = %k8s_err, "failed to create Job in Kubernetes – falling back to local headless execution");

            // Attempt local execution. Propagate any errors to the caller so
            // they can be tracked in metrics.
            self.openhands_client.process_task(&task).await?;
        }

        Ok(())
    }

    pub fn get_metrics(&self) -> TaskMetrics {
        TaskMetrics {
            tasks_total: self.metrics.tasks_total,
            tasks_running: self.metrics.tasks_running,
            tasks_succeeded: self.metrics.tasks_succeeded,
            tasks_failed: self.metrics.tasks_failed,
        }
    }
} 