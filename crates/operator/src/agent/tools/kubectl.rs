//! Kubectl Tool for Kubernetes Operations
//! 
//! Provides safe kubectl command execution for agent investigations.

use super::{ToolResult, ToolArgs, ToolError};
use anyhow::Result;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::{Api, ListParams}, Client};
use rig::completion::ToolDefinition;
use rig::tool::Tool as RigTool;
use regex::Regex;
use std::collections::HashSet;
use tokio;

/// Kubectl tool for Kubernetes operations
#[derive(Clone)]
pub struct KubectlTool {
    client: Client,
    allowed_verbs: HashSet<String>,
    namespace_whitelist: Option<Vec<String>>,
}

impl KubectlTool {
    pub fn new(client: Client) -> Self {
        let mut allowed_verbs = HashSet::new();
        // Safe read-only operations
        allowed_verbs.insert("get".to_string());
        allowed_verbs.insert("describe".to_string());
        allowed_verbs.insert("logs".to_string());
        allowed_verbs.insert("top".to_string());
        allowed_verbs.insert("events".to_string());
        
        Self {
            client,
            allowed_verbs,
            namespace_whitelist: None,
        }
    }
    
    /// Add additional allowed verbs (for remediation workflows)
    pub fn with_allowed_verbs(mut self, verbs: Vec<String>) -> Self {
        self.allowed_verbs.extend(verbs);
        self
    }
    
    /// Restrict to specific namespaces
    pub fn with_namespace_whitelist(mut self, namespaces: Vec<String>) -> Self {
        self.namespace_whitelist = Some(namespaces);
        self
    }
    
    /// Parse kubectl command into components
    fn parse_command(&self, cmd: &str) -> Result<KubectlCommand> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        
        if parts.is_empty() || parts[0] != "kubectl" {
            return Err(anyhow::anyhow!("Command must start with 'kubectl'"));
        }
        
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Missing kubectl verb"));
        }
        
        let verb = parts[1];
        if !self.allowed_verbs.contains(verb) {
            return Err(anyhow::anyhow!("Verb '{}' is not allowed", verb));
        }
        
        // Extract namespace if specified
        let namespace = if let Some(ns_idx) = parts.iter().position(|&p| p == "-n" || p == "--namespace") {
            parts.get(ns_idx + 1).map(|s| s.to_string())
        } else {
            None
        };
        
        // Validate namespace if whitelist is configured
        if let Some(ref whitelist) = self.namespace_whitelist {
            if let Some(ref ns) = namespace {
                if !whitelist.contains(ns) {
                    return Err(anyhow::anyhow!("Namespace '{}' is not in whitelist", ns));
                }
            }
        }
        
        Ok(KubectlCommand {
            verb: verb.to_string(),
            resource: parts.get(2).map(|s| s.to_string()),
            name: parts.get(3).map(|s| s.to_string()),
            namespace,
            full_command: cmd.to_string(),
        })
    }
    
    /// Execute kubectl command via Kubernetes API
    async fn execute_command(&self, cmd: &KubectlCommand) -> Result<String> {
        match cmd.verb.as_str() {
            "get" => self.execute_get(cmd).await,
            "describe" => self.execute_describe(cmd).await,
            "logs" => self.execute_logs(cmd).await,
            "top" => Ok("Top command not yet implemented".to_string()),
            "events" => Ok("Events command not yet implemented".to_string()),
            _ => Err(anyhow::anyhow!("Unsupported verb: {}", cmd.verb)),
        }
    }
    
    async fn execute_get(&self, cmd: &KubectlCommand) -> Result<String> {
        let resource = cmd.resource.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing resource type"))?;
        
        match resource.as_str() {
            "pods" | "pod" => {
                let namespace = cmd.namespace.as_deref().unwrap_or("default");
                let pods: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
                
                if let Some(name) = &cmd.name {
                    // Get specific pod
                    match pods.get(name).await {
                        Ok(pod) => Ok(serde_json::to_string_pretty(&pod)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get pod: {}", e)),
                    }
                } else {
                    // List all pods
                    match pods.list(&ListParams::default()).await {
                        Ok(pod_list) => {
                            let summary: Vec<String> = pod_list.items.iter().map(|pod| {
                                format!("{}\t{}\t{}", 
                                    pod.metadata.name.as_ref().unwrap_or(&"<unknown>".to_string()),
                                    pod.status.as_ref()
                                        .and_then(|s| s.phase.as_ref())
                                        .unwrap_or(&"Unknown".to_string()),
                                    pod.metadata.creation_timestamp.as_ref()
                                        .map(|t| t.0.to_string())
                                        .unwrap_or_else(|| "<unknown>".to_string())
                                )
                            }).collect();
                            Ok(format!("NAME\tSTATUS\tAGE\n{}", summary.join("\n")))
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list pods: {}", e)),
                    }
                }
            }
            _ => Ok(format!("Resource type '{}' not yet implemented", resource)),
        }
    }
    
    async fn execute_describe(&self, _cmd: &KubectlCommand) -> Result<String> {
        Ok("Describe command not yet implemented".to_string())
    }
    
    async fn execute_logs(&self, _cmd: &KubectlCommand) -> Result<String> {
        Ok("Logs command not yet implemented".to_string())
    }
    
    /// Validate if the command is safe to execute
    fn validate(&self, input: &str) -> Result<()> {
        // Check for dangerous patterns
        let dangerous_patterns = vec![
            r";\s*rm\s+",
            r"&&\s*rm\s+",
            r"\|\s*rm\s+",
            r"delete\s+",
            r"exec\s+",
            r"apply\s+",
            r"patch\s+",
            r"scale\s+",
            r"--force",
            r"-f\s+/",
        ];
        
        let input_lower = input.to_lowercase();
        for pattern in dangerous_patterns {
            let re = Regex::new(pattern)?;
            if re.is_match(&input_lower) {
                return Err(anyhow::anyhow!("Command contains dangerous pattern: {}", pattern));
            }
        }
        
        Ok(())
    }
}

// Implement Rig's Tool trait
impl RigTool for KubectlTool {
    const NAME: &'static str = "kubectl";
    
    type Error = ToolError;
    type Args = ToolArgs;
    type Output = ToolResult;
    
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute kubectl commands for Kubernetes cluster inspection and management. \
                         Supports get, describe, logs, top, and optionally delete/scale operations.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The kubectl command to execute (e.g., 'kubectl get pods -n default')"
                    }
                },
                "required": ["command"]
            }),
        }
    }
    
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Validate the command
        self.validate(&args.command)
            .map_err(|e| ToolError::ValidationError(e.to_string()))?;
        
        // Parse command
        let cmd = match self.parse_command(&args.command) {
            Ok(cmd) => cmd,
            Err(e) => return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                metadata: None,
            }),
        };
        
        // Clone self for the spawned task
        let tool = self.clone();
        
        // Spawn the execution to avoid Sync issues with kube client
        let result = tokio::spawn(async move {
            tool.execute_command(&cmd).await
        })
        .await
        .map_err(|e| ToolError::InternalError(anyhow::anyhow!("Task join error: {}", e)))?;
        
        match result {
            Ok(output) => Ok(ToolResult {
                success: true,
                output,
                error: None,
                metadata: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                metadata: None,
            }),
        }
    }
}

#[derive(Debug)]
struct KubectlCommand {
    verb: String,
    resource: Option<String>,
    name: Option<String>,
    namespace: Option<String>,
    full_command: String,
} 