use std::sync::Arc;
use crate::{
    openhands::OpenHandsClient,
    server::Alert,
    store::Store,
    TaskMetrics,
    Result,
    config::TaskExecutionMode,
};

pub struct TaskScheduler {
    kube_client: kube::Client,
    openhands_client: Arc<OpenHandsClient>,
    store: Arc<dyn Store>,
    metrics: TaskMetrics,
    execution_mode: TaskExecutionMode,
}

impl TaskScheduler {
    pub fn new(
        kube_client: kube::Client,
        openhands_client: Arc<OpenHandsClient>,
        store: Arc<dyn Store>,
        execution_mode: TaskExecutionMode,
    ) -> Self {
        Self {
            kube_client,
            openhands_client,
            store,
            metrics: TaskMetrics::default(),
            execution_mode,
        }
    }

    // TODO: Phase 1 - Replace with workflow engine
    pub async fn schedule_task(&mut self, _alert: Alert, _task_id: uuid::Uuid) -> Result<()> {
        tracing::warn!("Task scheduling not yet implemented for Phase 1");
        Ok(())
    }

    /*
    // Old implementation commented out for Phase 1 rewrite
    pub async fn schedule_task(&mut self, _alert: Alert, task_record: TaskRecord) -> Result<()> {
        // Convert TaskRecord to the Task format used by OpenHands client
        let task = Task {
            id: task_record.id.to_string(),
            prompt: task_record.prompt.clone(),
            model: if task_record.model.is_empty() { None } else { Some(task_record.model.clone()) },
            max_retries: Some(task_record.max_retries),
            timeout: Some(task_record.timeout),
            resources: crate::TaskResources {
                cpu_limit: task_record.resources.cpu_limit.clone(),
                memory_limit: task_record.resources.memory_limit.clone(),
                cpu_request: task_record.resources.cpu_request.clone(),
                memory_request: task_record.resources.memory_request.clone(),
            },
        };

        self.metrics.tasks_total += 1;
        self.metrics.tasks_running += 1;

        match self.execution_mode {
            TaskExecutionMode::Local => {
                // Execute directly in-process (optionally in its own task)
                // Here we simply await the completion; callers may run the
                // scheduler on a dedicated Tokio runtime/thread pool.
                self.openhands_client.process_task(&task, task_record.id).await?;
            }
            TaskExecutionMode::Kubernetes => {
                // Try to offload to Kubernetes Job, fall back to local on
                // failure (e.g. when running outside a cluster).
                if let Some(kube_client) = &self.kube_client {
                    if let Err(k8s_err) = kube_client.create_task_job(&task).await {
                        tracing::warn!(error = %k8s_err, "failed to create Job in Kubernetes – falling back to local headless execution");

                        // Attempt local execution. Propagate any errors to the
                        // caller so they can be tracked in metrics.
                        self.openhands_client.process_task(&task, task_record.id).await?;
                    }
                } else {
                    tracing::warn!("Kubernetes execution mode requested but no Kubernetes client available – falling back to local execution");
                    self.openhands_client.process_task(&task, task_record.id).await?;
                }
            }
        }

        Ok(())
    }
    */

    pub fn get_metrics(&self) -> TaskMetrics {
        TaskMetrics {
            tasks_total: self.metrics.tasks_total,
            tasks_running: self.metrics.tasks_running,
            tasks_succeeded: self.metrics.tasks_succeeded,
            tasks_failed: self.metrics.tasks_failed,
        }
    }
} 