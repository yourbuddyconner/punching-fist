use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::{controller::{Action, Controller}, watcher::Config},
    Client, ResourceExt,
};
use serde_json::json;
use tracing::{error, info, warn};

use crate::{
    crd::{Workflow, WorkflowStatus},
    store::Store,
    workflow::WorkflowEngine,
    Error, Result,
};

pub struct WorkflowController {
    client: Client,
    store: Arc<dyn Store>,
    engine: Arc<WorkflowEngine>,
}

impl WorkflowController {
    pub fn new(client: Client, store: Arc<dyn Store>, engine: Arc<WorkflowEngine>) -> Self {
        Self { client, store, engine }
    }

    pub async fn run(self: Arc<Self>) {
        info!("Starting Workflow controller");

        let workflows: Api<Workflow> = Api::all(self.client.clone());
        
        Controller::new(workflows.clone(), Config::default())
            .run(Self::reconcile, Self::error_policy, self)
            .for_each(|res| async move {
                match res {
                    Ok((_workflow, _action)) => {}
                    Err(e) => error!("Reconciliation error: {}", e),
                }
            })
            .await;
    }

    async fn reconcile(workflow: Arc<Workflow>, ctx: Arc<Self>) -> Result<Action> {
        let name = workflow.name_any();
        let namespace = workflow.namespace().unwrap_or_else(|| "default".to_string());
        
        info!("Reconciling Workflow: {}/{}", namespace, name);

        // Get the current status or initialize it
        let status = workflow.status.as_ref();
        
        match status.map(|s| s.phase.as_str()) {
            None | Some("") => {
                // New workflow, start execution
                ctx.start_workflow(&workflow).await?;
                Ok(Action::requeue(Duration::from_secs(1)))
            }
            Some("Pending") => {
                // Workflow is pending, check if we should start
                ctx.check_pending_workflow(&workflow).await?;
                Ok(Action::requeue(Duration::from_secs(5)))
            }
            Some("Running") => {
                // Workflow is running, check progress
                ctx.check_running_workflow(&workflow).await?;
                Ok(Action::requeue(Duration::from_secs(5)))
            }
            Some("Succeeded") | Some("Failed") => {
                // Terminal state, no more reconciliation needed
                Ok(Action::await_change())
            }
            Some(phase) => {
                warn!("Unknown workflow phase: {}", phase);
                Ok(Action::requeue(Duration::from_secs(30)))
            }
        }
    }

    async fn start_workflow(&self, workflow: &Workflow) -> Result<()> {
        let name = workflow.name_any();
        let namespace = workflow.namespace().unwrap_or_else(|| "default".to_string());
        
        info!("Starting workflow execution: {}/{}", namespace, name);

        // Update status to Pending
        self.update_status(workflow, "Pending", "Workflow queued for execution", None).await?;

        // Queue the workflow for execution
        self.engine.queue_workflow(workflow.clone()).await?;

        Ok(())
    }

    async fn check_pending_workflow(&self, workflow: &Workflow) -> Result<()> {
        let name = workflow.name_any();
        let namespace = workflow.namespace().unwrap_or_else(|| "default".to_string());

        // For now, we'll check by phase since we don't have execution_id in the status
        if let Some(status) = &workflow.status {
            if status.phase == "Pending" {
                // Workflow is still pending, will be picked up by engine
                info!("Workflow {}/{} is pending execution", namespace, name);
            }
        }

        Ok(())
    }

    async fn check_running_workflow(&self, workflow: &Workflow) -> Result<()> {
        let name = workflow.name_any();
        let namespace = workflow.namespace().unwrap_or_else(|| "default".to_string());

        // For now, we'll check by name and namespace
        // In a real implementation, we'd track execution ID in metadata or annotations
        info!("Checking running workflow {}/{}", namespace, name);

        Ok(())
    }

    async fn update_status(
        &self,
        workflow: &Workflow,
        phase: &str,
        message: &str,
        outputs: Option<serde_json::Value>,
    ) -> Result<()> {
        let name = workflow.name_any();
        let namespace = workflow.namespace().unwrap_or_else(|| "default".to_string());
        
        let api: Api<Workflow> = Api::namespaced(self.client.clone(), &namespace);
        
        let status = WorkflowStatus {
            phase: phase.to_string(),
            start_time: workflow.status.as_ref()
                .and_then(|s| s.start_time.clone())
                .or_else(|| Some(chrono::Utc::now().to_rfc3339())),
            completion_time: if phase == "Succeeded" || phase == "Failed" {
                Some(chrono::Utc::now().to_rfc3339())
            } else {
                workflow.status.as_ref().and_then(|s| s.completion_time.clone())
            },
            steps: workflow.status.as_ref()
                .map(|s| s.steps.clone())
                .unwrap_or_default(),
            outputs: outputs
                .and_then(|v| v.as_object().cloned())
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            error: if phase == "Failed" {
                Some(message.to_string())
            } else {
                None
            },
            conditions: vec![],
        };

        let patch = json!({
            "status": status
        });

        api.patch_status(
            &name,
            &PatchParams::default(),
            &Patch::Merge(patch),
        ).await
            .map_err(|e| Error::Kubernetes(e.to_string()))?;

        info!("Updated workflow {}/{} status to {}", namespace, name, phase);
        Ok(())
    }

    fn error_policy(_workflow: Arc<Workflow>, error: &Error, _ctx: Arc<Self>) -> Action {
        error!("Workflow reconciliation error: {}", error);
        Action::requeue(Duration::from_secs(30))
    }
} 