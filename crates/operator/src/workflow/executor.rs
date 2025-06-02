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
use tera::{Tera, Context as TeraContext};
use regex;

use crate::{
    crd::{WorkflowStep, StepType},
    workflow::WorkflowContext,
    agent::{AgentRuntime, LLMConfig, tools::{kubectl::KubectlTool, promql::PromQLTool, curl::CurlTool, script::ScriptTool}, provider::map_anthropic_model},
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

        // Get LLM config from context or use defaults
        let mut llm_config = if let Some(config_value) = context.get_metadata("llm_config") {
            serde_json::from_value(config_value.clone())
                .unwrap_or_else(|_| LLMConfig::default())
        } else {
            LLMConfig::default()
        };

        // Apply model mapping for Anthropic models to ensure correct API identifiers
        if llm_config.provider == "anthropic" || llm_config.provider == "claude" {
            let mapped_model = map_anthropic_model(&llm_config.model);
            if mapped_model != llm_config.model {
                info!("Mapped model '{}' to '{}' for Anthropic API", llm_config.model, mapped_model);
                llm_config.model = mapped_model.to_string();
            }
        }

        // Create agent runtime
        let mut agent_runtime = AgentRuntime::new(llm_config)
            .map_err(|e| Error::Internal(format!("Failed to create agent runtime: {}", e)))?;

        // Add tools based on step configuration
        if !step.tools.is_empty() {
            for tool in &step.tools {
                // Extract tool name from the Tool enum
                let tool_name = match tool {
                    crate::crd::Tool::Named(name) => name.as_str(),
                    crate::crd::Tool::Detailed(detailed) => detailed.name.as_str(),
                };
                
                match tool_name {
                    "kubectl" => {
                        let kubectl_tool = KubectlTool::new(self.client.clone());
                        agent_runtime.add_tool("kubectl".to_string(), kubectl_tool);
                    }
                    "promql" => {
                        let prometheus_url = context.get_metadata("prometheus_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("http://prometheus:9090")
                            .to_string();
                        let promql_tool = PromQLTool::new(prometheus_url);
                        agent_runtime.add_tool("promql".to_string(), promql_tool);
                    }
                    "curl" => {
                        let curl_tool = CurlTool::new();
                        agent_runtime.add_tool("curl".to_string(), curl_tool);
                    }
                    "script" => {
                        let script_tool = ScriptTool::new();
                        agent_runtime.add_tool("script".to_string(), script_tool);
                    }
                    _ => {
                        warn!("Unknown tool requested: {}", tool_name);
                    }
                }
            }
        }

        // Build investigation context
        let mut investigation_context = std::collections::HashMap::new();
        
        // Add alert context if available
        if let Some(alert_name) = context.get_metadata("alert_name").and_then(|v| v.as_str()) {
            investigation_context.insert("alert_name".to_string(), alert_name.to_string());
        }
        if let Some(severity) = context.get_metadata("severity").and_then(|v| v.as_str()) {
            investigation_context.insert("severity".to_string(), severity.to_string());
        }
        
        // Add step inputs to context
        if let Some(inputs) = context.get_template_context().get("input").and_then(|v| v.as_object()) {
            for (key, value) in inputs {
                if let Some(str_value) = value.as_str() {
                    investigation_context.insert(key.clone(), str_value.to_string());
                }
            }
        }

        // Render goal with template values
        let rendered_goal = self.render_template(goal, context)?;

        // Execute investigation with timeout
        let timeout_duration = Duration::from_secs(step.timeout_minutes.unwrap_or(10) as u64 * 60);
        match timeout(timeout_duration, agent_runtime.investigate(&rendered_goal, investigation_context)).await {
            Ok(Ok(agent_result)) => {
                info!("Agent step {} completed successfully", step.name);
                
                // Convert agent result to step result
                Ok(StepResult {
                    output: serde_json::json!({
                        "summary": agent_result.summary,
                        "findings": agent_result.findings,
                        "root_cause": agent_result.root_cause,
                        "confidence": agent_result.confidence,
                        "actions_taken": agent_result.actions_taken,
                        "recommendations": agent_result.recommendations,
                        "can_auto_fix": agent_result.can_auto_fix,
                        "fix_command": agent_result.fix_command,
                        "escalation_notes": agent_result.escalation_notes,
                        "report": agent_result.format_report(),
                    }),
                    success: true,
                })
            }
            Ok(Err(e)) => {
                error!("Agent step {} failed: {}", step.name, e);
                Ok(StepResult {
                    output: serde_json::json!({
                        "error": e.to_string(),
                        "goal": rendered_goal,
                    }),
                    success: false,
                })
            }
            Err(_) => {
                error!("Agent step {} timed out", step.name);
                Ok(StepResult {
                    output: serde_json::json!({
                        "error": "Agent investigation timed out",
                        "goal": rendered_goal,
                    }),
                    success: false,
                })
            }
        }
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
        let template_context = context.get_template_context();
        crate::template::render_template(template, &template_context)
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

        // Use Tera to evaluate the path
        let path_template = format!("{{{{ {} }}}}", path);
        let actual_value = self.render_template(&path_template, context)
            .unwrap_or_else(|_| String::new());

        match operator {
            "==" => Ok(actual_value == expected),
            "!=" => Ok(actual_value != expected),
            _ => Err(Error::Validation(format!("Unknown operator: {}", operator))),
        }
    }
} 