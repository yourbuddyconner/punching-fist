use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, PostParams, WatchEvent, WatchParams},
    Client,
};
use serde_json::Value;
use tokio::time::timeout;
use tracing::{error, info, warn};
use futures::{StreamExt, TryStreamExt};

use crate::{
    crd::{WorkflowStep, StepType},
    workflow::WorkflowContext,
    Result, Error,
};

#[derive(Debug, Clone)]
pub struct StepResult {
    pub output: Value,
    pub success: bool,
}

pub struct StepExecutor {
    client: Client,
    namespace: String,
}

impl StepExecutor {
    pub fn new(client: Client, namespace: String) -> Self {
        Self { client, namespace }
    }

    pub async fn execute_step(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> Result<StepResult> {
        info!("Executing step: {} (type: {:?})", step.name, step.step_type);

        match step.step_type {
            StepType::Cli => {
                self.execute_cli_step(step, context).await
            }
            StepType::Agent => {
                self.execute_agent_step(step, context).await
            }
            StepType::Conditional => {
                self.execute_conditional_step(step, context).await
            }
        }
    }

    async fn execute_cli_step(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> Result<StepResult> {
        info!("Executing CLI step: {}", step.name);

        let command = step.command.as_ref()
            .ok_or_else(|| Error::Validation("CLI step missing command".to_string()))?;

        // Render command with context
        let rendered_command = self.render_template(command, context)?;
        
        // Get runtime config from context metadata (should be set by workflow engine)
        let image = context.get_metadata("runtime_image")
            .and_then(|v| v.as_str())
            .unwrap_or("busybox:latest")
            .to_string();
        
        // Create a pod to execute the command
        let pod_name = format!("workflow-cli-{}-{}", step.name.to_lowercase().replace(" ", "-"), uuid::Uuid::new_v4());
        let pod = self.create_cli_pod(&pod_name, &image, &rendered_command, &Default::default())?;

        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
        
        // Create the pod
        pods.create(&PostParams::default(), &pod).await
            .map_err(|e| Error::Kubernetes(e.to_string()))?;

        // Wait for pod completion with timeout
        let timeout_duration = Duration::from_secs(step.timeout_minutes.unwrap_or(5) as u64 * 60);
        match timeout(timeout_duration, self.wait_for_pod_completion(&pod_name)).await {
            Ok(Ok(output)) => {
                info!("CLI step {} completed successfully", step.name);
                Ok(StepResult {
                    output: serde_json::json!({
                        "stdout": output,
                        "command": rendered_command,
                    }),
                    success: true,
                })
            }
            Ok(Err(e)) => {
                error!("CLI step {} failed: {}", step.name, e);
                Ok(StepResult {
                    output: serde_json::json!({
                        "error": e.to_string(),
                        "command": rendered_command,
                    }),
                    success: false,
                })
            }
            Err(_) => {
                error!("CLI step {} timed out", step.name);
                Ok(StepResult {
                    output: serde_json::json!({
                        "error": "Command timed out",
                        "command": rendered_command,
                    }),
                    success: false,
                })
            }
        }
    }

    async fn execute_agent_step(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> Result<StepResult> {
        info!("Executing Agent step: {}", step.name);

        let goal = step.goal.as_ref()
            .ok_or_else(|| Error::Validation("Agent step missing goal".to_string()))?;

        // For Week 4, we'll create a placeholder that shows what will happen
        // Week 5 will implement the actual LLM agent runtime
        
        warn!("Agent step execution not yet implemented (coming in Week 5)");
        
        Ok(StepResult {
            output: serde_json::json!({
                "message": "Agent step placeholder - will be implemented in Week 5",
                "goal": goal,
                "tools": step.tools,
                "max_iterations": step.max_iterations,
            }),
            success: true,
        })
    }

    async fn execute_conditional_step(
        &self,
        step: &WorkflowStep,
        context: &WorkflowContext,
    ) -> Result<StepResult> {
        info!("Executing Conditional step: {}", step.name);

        let condition = step.condition.as_ref()
            .ok_or_else(|| Error::Validation("Conditional step missing condition".to_string()))?;

        // Evaluate the condition
        let condition_met = self.evaluate_condition(condition, context)?;

        let result = if condition_met {
            serde_json::json!({
                "condition_met": true,
                "branch": "then",
                "message": format!("Condition '{}' evaluated to true", condition),
            })
        } else {
            serde_json::json!({
                "condition_met": false,
                "branch": "else",
                "message": format!("Condition '{}' evaluated to false", condition),
            })
        };

        Ok(StepResult {
            output: result,
            success: true,
        })
    }

    fn create_cli_pod(
        &self,
        name: &str,
        image: &str,
        command: &str,
        env: &std::collections::HashMap<String, String>,
    ) -> Result<Pod> {
        use k8s_openapi::api::core::v1::{Container, EnvVar, PodSpec};
        
        let env_vars: Vec<EnvVar> = env.iter()
            .map(|(k, v)| EnvVar {
                name: k.clone(),
                value: Some(v.clone()),
                ..Default::default()
            })
            .collect();

        let pod = Pod {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(name.to_string()),
                labels: Some([
                    ("app".to_string(), "punching-fist".to_string()),
                    ("component".to_string(), "workflow-cli".to_string()),
                ].iter().cloned().collect()),
                ..Default::default()
            },
            spec: Some(PodSpec {
                containers: vec![Container {
                    name: "cli".to_string(),
                    image: Some(image.to_string()),
                    command: Some(vec!["/bin/sh".to_string()]),
                    args: Some(vec!["-c".to_string(), command.to_string()]),
                    env: Some(env_vars),
                    ..Default::default()
                }],
                restart_policy: Some("Never".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        Ok(pod)
    }

    async fn wait_for_pod_completion(&self, pod_name: &str) -> Result<String> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
        
        // Watch for pod status changes
        let wp = WatchParams::default()
            .fields(&format!("metadata.name={}", pod_name))
            .timeout(300);

        let mut stream = pods.watch(&wp, "0").await
            .map_err(|e| Error::Kubernetes(e.to_string()))?
            .boxed();

        while let Some(event) = stream.try_next().await
            .map_err(|e| Error::Kubernetes(e.to_string()))? {
            
            match event {
                WatchEvent::Modified(pod) => {
                    if let Some(status) = &pod.status {
                        if let Some(phase) = &status.phase {
                            match phase.as_str() {
                                "Succeeded" => {
                                    // Get logs
                                    let logs = self.get_pod_logs(pod_name).await?;
                                    return Ok(logs);
                                }
                                "Failed" => {
                                    let logs = self.get_pod_logs(pod_name).await?;
                                    return Err(Error::Execution(format!("Pod failed: {}", logs)));
                                }
                                _ => continue,
                            }
                        }
                    }
                }
                _ => continue,
            }
        }

        Err(Error::Execution("Pod watch ended without completion".to_string()))
    }

    async fn get_pod_logs(&self, pod_name: &str) -> Result<String> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);
        
        pods.logs(pod_name, &Default::default()).await
            .map_err(|e| Error::Kubernetes(e.to_string()))
    }

    fn render_template(&self, template: &str, context: &WorkflowContext) -> Result<String> {
        // Simple template rendering - replace {{path.to.value}} with actual values
        let mut result = template.to_string();
        let template_context = context.get_template_context();

        // Find all template variables
        let re = regex::Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        for cap in re.captures_iter(template) {
            if let Some(path) = cap.get(1) {
                let path_str = path.as_str().trim();
                if let Some(value) = self.get_value_by_path(&template_context, path_str) {
                    let value_str = match value {
                        Value::String(s) => s.clone(),
                        _ => value.to_string(),
                    };
                    result = result.replace(&format!("{{{{{}}}}}", path_str), &value_str);
                }
            }
        }

        Ok(result)
    }

    fn get_value_by_path<'a>(&self, value: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            match current {
                Value::Object(map) => {
                    current = map.get(part)?;
                }
                Value::Array(arr) => {
                    let index: usize = part.parse().ok()?;
                    current = arr.get(index)?;
                }
                _ => return None,
            }
        }

        Some(current)
    }

    fn evaluate_condition(&self, condition: &str, context: &WorkflowContext) -> Result<bool> {
        // Simple condition evaluation
        // Format: "path.to.value == expected" or "path.to.value != expected"
        
        let parts: Vec<&str> = condition.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(Error::Validation(format!("Invalid condition format: {}", condition)));
        }

        let path = parts[0];
        let operator = parts[1];
        let expected = parts[2].trim_matches('"').trim_matches('\'');

        let template_context = context.get_template_context();
        let actual = self.get_value_by_path(&template_context, path);

        match operator {
            "==" => {
                if let Some(value) = actual {
                    match value {
                        Value::String(s) => Ok(s == expected),
                        Value::Bool(b) => Ok(b.to_string() == expected),
                        Value::Number(n) => Ok(n.to_string() == expected),
                        _ => Ok(false),
                    }
                } else {
                    Ok(false)
                }
            }
            "!=" => {
                if let Some(value) = actual {
                    match value {
                        Value::String(s) => Ok(s != expected),
                        Value::Bool(b) => Ok(b.to_string() != expected),
                        Value::Number(n) => Ok(n.to_string() != expected),
                        _ => Ok(true),
                    }
                } else {
                    Ok(true)
                }
            }
            _ => Err(Error::Validation(format!("Unknown operator: {}", operator))),
        }
    }
} 