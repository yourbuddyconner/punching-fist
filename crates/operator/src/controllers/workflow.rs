use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::{
    api::{Api, Patch, PatchParams},
    runtime::{controller::{Action, Controller}, watcher::Config},
    Client, ResourceExt,
};
use serde_json::json;
use tracing::{error, info, warn, debug};

use crate::{
    crd::{Workflow, WorkflowStatus, common::EventContext, common::WorkflowInfo, common::SourceInfo, sink::Sink},
    store::Store,
    workflow::WorkflowEngine,
    Error, Result,
    controllers::SinkController,
};

pub struct WorkflowController {
    client: Client,
    store: Arc<dyn Store>,
    engine: Arc<WorkflowEngine>,
    sink_controller: Arc<SinkController>,
}

impl WorkflowController {
    pub fn new(
        client: Client, 
        store: Arc<dyn Store>, 
        engine: Arc<WorkflowEngine>, 
        sink_controller: Arc<SinkController>
    ) -> Self {
        Self { client, store, engine, sink_controller }
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
        
        // Get the current status or initialize it
        let status = workflow.status.as_ref();
        
        match status.map(|s| s.phase.as_str()) {
            None | Some("") => {
                // New workflow, start execution
                info!("Registering new Workflow resource: {}/{}", namespace, name);
                info!(
                    "Workflow '{}' has {} step(s) configured",
                    name,
                    workflow.spec.steps.len()
                );
                ctx.start_workflow(&workflow).await?;
                Ok(Action::requeue(Duration::from_secs(1)))
            }
            Some("Pending") => {
                // Workflow is pending, check if we should start
                debug!("Reconciling pending Workflow: {}/{}", namespace, name);
                ctx.check_pending_workflow(&workflow).await?;
                Ok(Action::requeue(Duration::from_secs(5)))
            }
            Some("Running") => {
                // Workflow is running, check progress
                debug!("Reconciling running Workflow: {}/{}", namespace, name);
                ctx.check_running_workflow(&workflow).await?;
                Ok(Action::requeue(Duration::from_secs(5)))
            }
            Some("Succeeded") => {
                // Terminal state, no more reconciliation needed
                // BUT, if it just succeeded, we need to process sinks.
                // We'll add a helper for this.
                info!("Workflow {}/{} completed successfully", namespace, name);
                ctx.process_succeeded_workflow(&workflow).await?;
                Ok(Action::await_change())
            }
            Some("Failed") => {
                // Terminal state, no more reconciliation needed
                info!("Workflow {}/{} is in failed state", namespace, name);
                Ok(Action::await_change())
            }
            Some(phase) => {
                warn!("Unknown workflow phase '{}' for {}/{}", phase, namespace, name);
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
                debug!("Workflow {}/{} is pending execution", namespace, name);
            }
        }

        Ok(())
    }

    async fn check_running_workflow(&self, workflow: &Workflow) -> Result<()> {
        let name = workflow.name_any();
        let namespace = workflow.namespace().unwrap_or_else(|| "default".to_string());

        // For now, we'll check by name and namespace
        // In a real implementation, we'd track execution ID in metadata or annotations
        debug!("Checking running workflow {}/{}", namespace, name);

        Ok(())
    }

    async fn process_succeeded_workflow(&self, workflow_cr: &Workflow) -> Result<()> {
        let wf_name = workflow_cr.name_any();
        let wf_namespace = workflow_cr.namespace().unwrap_or_else(|| "default".to_string());

        info!("Processing sinks for successfully completed workflow: {}/{}", wf_namespace, wf_name);

        if workflow_cr.spec.sinks.is_empty() {
            info!("Workflow {}/{} has no sinks configured.", wf_namespace, wf_name);
            return Ok(());
        }

        let wf_status = match &workflow_cr.status {
            Some(s) => s,
            None => {
                warn!("Workflow {}/{} is Succeeded but has no status. Cannot process sinks.", wf_namespace, wf_name);
                return Ok(()); // Or return an error
            }
        };

        // Construct the output context
        // This is a simplified example. You might want to fetch SourceInfo from the store
        // or pass it through annotations/labels if it's not directly in Workflow CR.
        // For now, creating a placeholder SourceInfo.
        let source_info = SourceInfo {
            name: "unknown-source".to_string(), // Placeholder
            source_type: "unknown".to_string(), // Placeholder
            namespace: wf_namespace.clone(),    // Placeholder, assume same namespace
        };

        let workflow_info = WorkflowInfo {
            name: wf_name.clone(),
            namespace: wf_namespace.clone(),
            outputs: wf_status.outputs.clone(), // Assuming this is HashMap<String, String>
            duration: wf_status.completion_time.as_ref().and_then(|ct| {
                wf_status.start_time.as_ref().and_then(|st| {
                    let ct_dt = chrono::DateTime::parse_from_rfc3339(ct).ok()?;
                    let st_dt = chrono::DateTime::parse_from_rfc3339(st).ok()?;
                    Some((ct_dt - st_dt).num_seconds().to_string())
                })
            }),
            completed_at: wf_status.completion_time.clone(),
        };

        // The `data` field in EventContext is serde_json::Value. 
        // If it's original trigger data, it might need to be fetched or reconstructed.
        // For now, using an empty JSON object.
        let event_data = json!({}); 

        let output_context_for_sinks = EventContext {
            source: source_info, 
            workflow: Some(workflow_info),
            data: event_data, // Placeholder
            timestamp: chrono::Utc::now().to_rfc3339(), // Timestamp of sink processing
        };

        let context_value = match serde_json::to_value(&output_context_for_sinks) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to serialize workflow output context for {}/{}: {}", wf_namespace, wf_name, e);
                return Err(e.into());
            }
        };

        for sink_name in &workflow_cr.spec.sinks {
            info!("Dispatching to sink '{}' for workflow {}/{}", sink_name, wf_namespace, wf_name);
            match self.sink_controller.process_sink_event(
                sink_name,
                &wf_namespace, // Assuming sink is in the same namespace as workflow
                &context_value
            ).await {
                Ok(_) => info!("Successfully processed sink '{}' for workflow {}/{}", sink_name, wf_namespace, wf_name),
                Err(e) => error!(
                    "Error processing sink '{}' for workflow {}/{}: {}",
                    sink_name, wf_namespace, wf_name, e
                ),
            }
        }
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