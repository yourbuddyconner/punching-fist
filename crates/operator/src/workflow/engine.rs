use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{error, info};
use uuid::Uuid;

use crate::{
    crd::Workflow,
    store::Store,
    workflow::{StepExecutor, WorkflowContext, WorkflowState},
    Result,
};

pub struct WorkflowEngine {
    store: Arc<dyn Store>,
    executor: Arc<StepExecutor>,
    executions: Arc<RwLock<HashMap<String, WorkflowExecution>>>,
    queue_tx: mpsc::Sender<Workflow>,
    queue_rx: Arc<RwLock<mpsc::Receiver<Workflow>>>,
}

struct WorkflowExecution {
    workflow: Workflow,
    state: WorkflowState,
    context: WorkflowContext,
    outputs: serde_json::Value,
}

impl WorkflowEngine {
    pub fn new(store: Arc<dyn Store>, executor: Arc<StepExecutor>) -> Self {
        let (queue_tx, queue_rx) = mpsc::channel(100);
        
        Self {
            store,
            executor,
            executions: Arc::new(RwLock::new(HashMap::new())),
            queue_tx,
            queue_rx: Arc::new(RwLock::new(queue_rx)),
        }
    }

    pub async fn start(self: Arc<Self>) {
        info!("Starting workflow engine");
        
        // Start the execution loop
        let engine = self.clone();
        tokio::spawn(async move {
            engine.execution_loop().await;
        });
    }

    async fn execution_loop(self: Arc<Self>) {
        let mut rx = self.queue_rx.write().await;
        
        while let Some(workflow) = rx.recv().await {
            let engine = self.clone();
            let execution_id = Uuid::new_v4().to_string();
            
            // Create execution record with properly populated context
            let mut context = WorkflowContext::new();
            
            // Add runtime configuration to context metadata
            context.add_metadata("runtime_image", serde_json::Value::String(workflow.spec.runtime.image.clone()));
            context.add_metadata("llm_config", serde_json::to_value(&workflow.spec.runtime.llm_config).unwrap_or_default());
            
            // Add environment variables to context
            for (key, value) in &workflow.spec.runtime.environment {
                context.add_metadata(&format!("env_{}", key), serde_json::Value::String(value.clone()));
            }
            
            // Parse and add source data from annotations
            if let Some(annotations) = &workflow.metadata.annotations {
                // Add alert metadata
                if let Some(alert_name) = annotations.get("alert.name") {
                    context.add_metadata("alert_name", serde_json::Value::String(alert_name.clone()));
                }
                if let Some(severity) = annotations.get("alert.severity") {
                    context.add_metadata("severity", serde_json::Value::String(severity.clone()));
                }
                
                // Parse and add source data for template rendering
                if let Some(source_data_str) = annotations.get("source.data") {
                    if let Ok(source_data) = serde_json::from_str::<serde_json::Value>(source_data_str) {
                        // Add source data to input context so templates can access it
                        let mut input = serde_json::Map::new();
                        input.insert("source".to_string(), serde_json::json!({
                            "data": source_data
                        }));
                        context.input = serde_json::Value::Object(input);
                    }
                }
            }
            
            let execution = WorkflowExecution {
                workflow: workflow.clone(),
                state: WorkflowState::Pending,
                context,
                outputs: serde_json::json!({}),
            };
            
            {
                let mut executions = engine.executions.write().await;
                executions.insert(execution_id.clone(), execution);
            }
            
            // Spawn execution task
            tokio::spawn(async move {
                if let Err(e) = engine.execute_workflow(&execution_id).await {
                    error!("Workflow execution failed: {}", e);
                }
            });
        }
    }

    async fn execute_workflow(&self, execution_id: &str) -> Result<()> {
        info!("Executing workflow: {}", execution_id);
        
        // Update state to Running
        {
            let mut executions = self.executions.write().await;
            if let Some(exec) = executions.get_mut(execution_id) {
                exec.state = WorkflowState::Running;
                
                // Store workflow in database
                let workflow_model = crate::store::Workflow {
                    id: Uuid::parse_str(execution_id).unwrap_or_else(|_| Uuid::new_v4()),
                    name: exec.workflow.metadata.name.clone().unwrap_or_else(|| "unnamed-workflow".to_string()),
                    namespace: exec.workflow.metadata.namespace.as_deref().unwrap_or("default").to_string(),
                    trigger_source: None,
                    status: crate::store::WorkflowStatus::Running,
                    steps_completed: 0,
                    total_steps: exec.workflow.spec.steps.len() as i32,
                    current_step: None,
                    input_context: Some(exec.context.to_json()),
                    outputs: None,
                    error: None,
                    started_at: chrono::Utc::now(),
                    completed_at: None,
                    created_at: chrono::Utc::now(),
                };
                self.store.save_workflow(workflow_model).await?;
            }
        }

        // Execute steps
        let workflow = {
            let executions = self.executions.read().await;
            executions.get(execution_id).map(|e| e.workflow.clone())
        };

        if let Some(workflow) = workflow {
            let mut step_outputs = HashMap::new();
            
            for (idx, step) in workflow.spec.steps.iter().enumerate() {
                info!("Executing step {}/{}: {}", idx + 1, workflow.spec.steps.len(), step.name);
                
                // Update current step
                {
                    let mut executions = self.executions.write().await;
                    if let Some(exec) = executions.get_mut(execution_id) {
                        exec.context.set_current_step(&step.name);
                    }
                }

                // Execute step
                let context = {
                    let executions = self.executions.read().await;
                    executions.get(execution_id).map(|e| e.context.clone())
                }.unwrap_or_else(WorkflowContext::new);

                match self.executor.execute_step(step, &context).await {
                    Ok(result) => {
                        info!("Step {} completed successfully", step.name);
                        
                        // Store step output
                        step_outputs.insert(step.name.clone(), result.output.clone());
                        
                        // Update context with output
                        let mut executions = self.executions.write().await;
                        if let Some(exec) = executions.get_mut(execution_id) {
                            exec.context.add_step_output(&step.name, result.output);
                        }
                    }
                    Err(e) => {
                        error!("Step {} failed: {}", step.name, e);
                        
                        // Update state to Failed
                        let mut executions = self.executions.write().await;
                        if let Some(exec) = executions.get_mut(execution_id) {
                            exec.state = WorkflowState::Failed;
                            exec.outputs = serde_json::json!({
                                "error": e.to_string(),
                                "failed_step": step.name,
                                "outputs": step_outputs,
                            });
                        }
                        
                        // Update database
                        let workflow_id = Uuid::parse_str(execution_id).unwrap_or_else(|_| Uuid::new_v4());
                        self.store.complete_workflow(
                            workflow_id,
                            crate::store::WorkflowStatus::Failed,
                            Some(serde_json::json!({
                                "error": e.to_string(),
                                "failed_step": step.name,
                                "outputs": step_outputs,
                            })),
                            Some(e.to_string()),
                        ).await?;
                        
                        return Err(e);
                    }
                }
                
                // Update database progress
                let workflow_id = Uuid::parse_str(execution_id).unwrap_or_else(|_| Uuid::new_v4());
                self.store.update_workflow_progress(
                    workflow_id,
                    idx as i32 + 1,
                    Some(step.name.clone()),
                ).await?;
            }
            
            // All steps completed successfully
            let outputs = serde_json::json!({ "steps": step_outputs });
            
            {
                let mut executions = self.executions.write().await;
                if let Some(exec) = executions.get_mut(execution_id) {
                    exec.state = WorkflowState::Succeeded;
                    exec.outputs = outputs.clone();
                }
            }
            
            // Update database
            let workflow_id = Uuid::parse_str(execution_id).unwrap_or_else(|_| Uuid::new_v4());
            self.store.complete_workflow(
                workflow_id,
                crate::store::WorkflowStatus::Succeeded,
                Some(outputs),
                None,
            ).await?;
        }

        Ok(())
    }

    pub async fn queue_workflow(&self, workflow: Workflow) -> Result<()> {
        self.queue_tx.send(workflow).await
            .map_err(|e| crate::Error::Internal(format!("Failed to queue workflow: {}", e)))?;
        Ok(())
    }

    pub async fn get_execution_status(&self, execution_id: &str) -> Result<Option<String>> {
        let executions = self.executions.read().await;
        Ok(executions.get(execution_id).map(|e| e.state.to_string()))
    }

    pub async fn get_execution_progress(&self, execution_id: &str) -> Result<serde_json::Value> {
        let executions = self.executions.read().await;
        if let Some(exec) = executions.get(execution_id) {
            Ok(serde_json::json!({
                "current_step": exec.context.current_step(),
                "state": exec.state.to_string(),
            }))
        } else {
            Ok(serde_json::json!({}))
        }
    }

    pub async fn get_execution_outputs(&self, execution_id: &str) -> Result<Option<serde_json::Value>> {
        let executions = self.executions.read().await;
        Ok(executions.get(execution_id).map(|e| e.outputs.clone()))
    }
} 