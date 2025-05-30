//! Kubectl Tool for Kubernetes Operations
//! 
//! Provides safe kubectl command execution for agent investigations.

use super::{Tool, ToolResult};
use anyhow::Result;
use async_trait::async_trait;
use k8s_openapi::api::core::v1::Pod;
use kube::{api::{Api, ListParams}, Client};
use regex::Regex;
use std::collections::HashSet;

/// Kubectl tool for Kubernetes operations
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
    fn parse_command(&self, command: &str) -> Result<KubectlCommand> {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() || parts[0] != "kubectl" {
            return Err(anyhow::anyhow!("Command must start with 'kubectl'"));
        }
        
        if parts.len() < 3 {
            return Err(anyhow::anyhow!("Incomplete kubectl command"));
        }
        
        let verb = parts[1].to_string();
        let resource = parts[2].to_string();
        
        // Parse namespace
        let namespace = if let Some(idx) = parts.iter().position(|&p| p == "-n" || p == "--namespace") {
            parts.get(idx + 1).map(|s| s.to_string())
        } else {
            None
        };
        
        // Parse resource name (if specified)
        let resource_name = if parts.len() > 3 && !parts[3].starts_with('-') {
            Some(parts[3].to_string())
        } else {
            None
        };
        
        // Parse additional flags
        let mut flags = Vec::new();
        let mut i = 3;
        if resource_name.is_some() {
            i = 4;
        }
        while i < parts.len() {
            if parts[i].starts_with('-') {
                flags.push(parts[i].to_string());
                if i + 1 < parts.len() && !parts[i + 1].starts_with('-') {
                    flags.push(parts[i + 1].to_string());
                    i += 1;
                }
            }
            i += 1;
        }
        
        Ok(KubectlCommand {
            verb,
            resource,
            resource_name,
            namespace,
            flags,
        })
    }
    
    /// Execute the parsed kubectl command
    async fn execute_command(&self, cmd: &KubectlCommand) -> Result<String> {
        match cmd.verb.as_str() {
            "get" => self.handle_get(cmd).await,
            "describe" => self.handle_describe(cmd).await,
            "logs" => self.handle_logs(cmd).await,
            "top" => self.handle_top(cmd).await,
            "delete" if self.allowed_verbs.contains("delete") => self.handle_delete(cmd).await,
            "scale" if self.allowed_verbs.contains("scale") => self.handle_scale(cmd).await,
            _ => Err(anyhow::anyhow!("Unsupported kubectl verb: {}", cmd.verb)),
        }
    }
    
    async fn handle_get(&self, cmd: &KubectlCommand) -> Result<String> {
        let namespace = cmd.namespace.as_deref().unwrap_or("default");
        
        match cmd.resource.as_str() {
            "pods" | "pod" => {
                let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
                
                if let Some(name) = &cmd.resource_name {
                    // Get specific pod
                    let pod = api.get(name).await?;
                    Ok(serde_json::to_string_pretty(&pod)?)
                } else {
                    // List pods
                    let pods = api.list(&ListParams::default()).await?;
                    Ok(serde_json::to_string_pretty(&pods)?)
                }
            }
            // Add more resource types as needed
            _ => Ok(format!("Resource type '{}' not yet implemented", cmd.resource)),
        }
    }
    
    async fn handle_describe(&self, _cmd: &KubectlCommand) -> Result<String> {
        // Implement describe functionality
        Ok("Describe functionality not yet implemented".to_string())
    }
    
    async fn handle_logs(&self, cmd: &KubectlCommand) -> Result<String> {
        if cmd.resource != "pod" && cmd.resource != "pods" {
            return Err(anyhow::anyhow!("Logs can only be retrieved for pods"));
        }
        
        let pod_name = cmd.resource_name.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pod name required for logs"))?;
        
        let namespace = cmd.namespace.as_deref().unwrap_or("default");
        let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        
        // Get logs (simplified - real implementation would handle more flags)
        let logs = api.logs(pod_name, &Default::default()).await?;
        Ok(logs)
    }
    
    async fn handle_top(&self, _cmd: &KubectlCommand) -> Result<String> {
        // Implement top functionality
        Ok("Top functionality not yet implemented".to_string())
    }
    
    async fn handle_delete(&self, _cmd: &KubectlCommand) -> Result<String> {
        // Implement delete functionality (with safety checks)
        Ok("Delete functionality not yet implemented".to_string())
    }
    
    async fn handle_scale(&self, _cmd: &KubectlCommand) -> Result<String> {
        // Implement scale functionality
        Ok("Scale functionality not yet implemented".to_string())
    }
}

#[async_trait]
impl Tool for KubectlTool {
    fn name(&self) -> &str {
        "kubectl"
    }
    
    fn description(&self) -> &str {
        "Execute kubectl commands for Kubernetes cluster inspection and management. \
         Supports get, describe, logs, top, and optionally delete/scale operations."
    }
    
    async fn execute(&self, input: &str) -> Result<ToolResult> {
        match self.parse_command(input) {
            Ok(cmd) => {
                match self.execute_command(&cmd).await {
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
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                metadata: None,
            }),
        }
    }
    
    fn validate(&self, input: &str) -> Result<()> {
        let cmd = self.parse_command(input)?;
        
        // Check if verb is allowed
        if !self.allowed_verbs.contains(&cmd.verb) {
            return Err(anyhow::anyhow!("Verb '{}' is not allowed", cmd.verb));
        }
        
        // Check namespace whitelist
        if let Some(whitelist) = &self.namespace_whitelist {
            let namespace = cmd.namespace.as_deref().unwrap_or("default");
            if !whitelist.contains(&namespace.to_string()) {
                return Err(anyhow::anyhow!("Namespace '{}' is not in whitelist", namespace));
            }
        }
        
        // Additional safety checks for destructive operations
        if cmd.verb == "delete" && cmd.resource == "namespace" {
            return Err(anyhow::anyhow!("Deleting namespaces is not allowed"));
        }
        
        Ok(())
    }
}

#[derive(Debug)]
struct KubectlCommand {
    verb: String,
    resource: String,
    resource_name: Option<String>,
    namespace: Option<String>,
    flags: Vec<String>,
} 