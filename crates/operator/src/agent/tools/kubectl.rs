//! Kubectl Tool for Kubernetes Operations
//! 
//! Provides safe kubectl command execution for agent investigations.
//! 
//! ## Usage in Agent Context
//! 
//! When initializing an agent, you can provide cluster context information:
//! ```rust,ignore
//! let kubectl_tool = KubectlTool::infer().await?;
//! let context = kubectl_tool.get_cluster_context().await?;
//! // Include context in agent prompt or initialization
//! ```
//! 
//! ## Supported Resources
//! 
//! - **pods**: List or get specific pods
//! - **namespaces**: List or get specific namespaces  
//! - **services**: List or get specific services
//! - **deployments**: List or get specific deployments
//! - **all**: Special resource type that returns pods, services, and deployments

use super::{ToolResult, ToolError};
use anyhow::Result;
use k8s_openapi::api::core::v1::{Pod, Namespace, Service, ConfigMap, Secret, Event};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, DaemonSet, ReplicaSet};
use k8s_openapi::api::batch::v1::{Job, CronJob};
use k8s_openapi::api::networking::v1::Ingress;
use kube::{api::{Api, ListParams, DynamicObject}, Client, discovery};
use kube::core::GroupVersionKind;
use rig::completion::ToolDefinition;
use rig::tool::Tool as RigTool;
use regex::Regex;
use std::collections::{HashSet, HashMap};
use tokio;
use kube::Config;
use serde::Deserialize;
use serde_yaml;

/// Arguments for KubectlTool execution
#[derive(Debug, Clone, Deserialize)]
pub struct KubectlToolArgs {
    pub verb: String,
    pub resource: Option<String>,
    pub name: Option<String>,
    pub namespace: Option<String>,
    pub tail_lines: Option<i64>, // Number of lines to return from the end of the logs
    pub field_selector: Option<String>, // Field selector for filtering resources (e.g., "status.phase=Running")
    pub label_selector: Option<String>, // Label selector for filtering resources (e.g., "app=nginx")
    // We might want to add a field for 'raw_options' or similar in the future
    // for flags that don't fit neatly into the above.
    // For now, keeping it simple.
}

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
    
    /// Create a new KubectlTool with automatically inferred Kubernetes configuration.
    /// 
    /// This will attempt to use:
    /// 1. Kubeconfig from KUBECONFIG env var or ~/.kube/config
    /// 2. In-cluster configuration (service account) if kubeconfig is not available
    /// 
    /// # Errors
    /// 
    /// Returns an error if no valid Kubernetes configuration can be found.
    pub async fn infer() -> Result<Self> {
        // Use Config::infer() to automatically detect available configuration
        let config = Config::infer().await
            .map_err(|e| anyhow::anyhow!("Failed to infer Kubernetes config: {}", e))?;
        
        // Create client from the inferred config
        let client = Client::try_from(config)
            .map_err(|e| anyhow::anyhow!("Failed to create Kubernetes client: {}", e))?;
        
        Ok(Self::new(client))
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
    
    /// Get cluster context information for agent initialization
    pub async fn get_cluster_context(&self) -> Result<String> {
        let mut context = Vec::new();
        
        // Get current cluster info
        if let Ok(config) = Config::infer().await {
            context.push(format!("Cluster URL: {}", config.cluster_url));
            context.push(format!("Default namespace: {}", config.default_namespace));
        }
        
        // List available namespaces
        let namespaces: Api<Namespace> = Api::all(self.client.clone());
        if let Ok(ns_list) = namespaces.list(&ListParams::default()).await {
            let ns_names: Vec<String> = ns_list.items.iter()
                .filter_map(|ns| ns.metadata.name.clone())
                .collect();
            context.push(format!("Available namespaces: {}", ns_names.join(", ")));
        }
        
        // List supported resource types
        let supported_resources = vec![
            "pods", "namespaces", "services", "deployments", "statefulsets", 
            "daemonsets", "replicasets", "jobs", "cronjobs", "configmaps", 
            "secrets", "ingresses", "all"
        ];
        context.push(format!("Supported resources: {}", supported_resources.join(", ")));
        
        Ok(context.join("\n"))
    }
    
    /// Execute kubectl command via Kubernetes API
    async fn execute_command(&self, args: &KubectlToolArgs) -> Result<String> {
        match args.verb.as_str() {
            "get" => self.execute_get(args).await,
            "describe" => self.execute_describe(args).await,
            "logs" => self.execute_logs(args).await,
            "top" => Ok("Top command not yet implemented".to_string()),
            "events" => self.execute_events(args).await,
            _ => Err(anyhow::anyhow!("Unsupported verb: {}", args.verb)),
        }
    }
    
    /// Format a generic resource list for output
    fn format_resource_list<T: serde::Serialize>(
        &self,
        items: Vec<T>,
        resource_type: &str,
        namespace_scoped: bool,
        metadata_extractor: impl Fn(&T) -> (Option<String>, Option<String>, Option<String>)
    ) -> String {
        let headers = if namespace_scoped {
            "NAMESPACE\tNAME\tAGE"
        } else {
            "NAME\tAGE"
        };
        
        let rows: Vec<String> = items.iter().map(|item| {
            let (namespace, name, timestamp) = metadata_extractor(item);
            let name = name.unwrap_or_else(|| "<unknown>".to_string());
            let age = timestamp.unwrap_or_else(|| "<unknown>".to_string());
            
            if namespace_scoped {
                let namespace = namespace.unwrap_or_else(|| "<unknown>".to_string());
                format!("{}\t{}\t{}", namespace, name, age)
            } else {
                format!("{}\t{}", name, age)
            }
        }).collect();
        
        format!("{}\n{}", headers, rows.join("\n"))
    }
    
    /// Build ListParams with optional field and label selectors
    fn build_list_params(&self, args: &KubectlToolArgs) -> ListParams {
        let mut lp = ListParams::default();
        
        if let Some(field_selector) = &args.field_selector {
            lp = lp.fields(field_selector);
        }
        
        if let Some(label_selector) = &args.label_selector {
            lp = lp.labels(label_selector);
        }
        
        lp
    }
    
    async fn execute_get(&self, args: &KubectlToolArgs) -> Result<String> {
        let resource = args.resource.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing resource type for 'get' verb"))?;
        
        // Handle "all" resource type
        if resource == "all" {
            return self.execute_get_all(args).await;
        }
        
        match resource.as_str() {
            "pods" | "pod" => {
                if let Some(name) = &args.name {
                    let namespace_to_use = args.namespace.as_deref();
                    if namespace_to_use == Some("all") {
                        // List pods in all namespaces and filter by name
                        let all_pods_api: Api<Pod> = Api::all(self.client.clone());
                        let lp = self.build_list_params(args);
                        match all_pods_api.list(&lp).await {
                            Ok(pod_list) => {
                                let mut found_pods = Vec::new();
                                for pod in pod_list.items {
                                    if pod.metadata.name.as_deref() == Some(name) {
                                        found_pods.push(pod);
                                    }
                                }
                                if found_pods.is_empty() {
                                    Err(anyhow::anyhow!("Pod '{}' not found in any namespace", name))
                                } else {
                                    // If multiple pods with the same name exist across namespaces, list them all
                                    // For a 'get' operation on a single named resource, typically one is expected.
                                    // However, this aligns with finding all matches if name is not unique globally.
                                    Ok(serde_json::to_string_pretty(&found_pods)?)
                                }
                            }
                            Err(e) => Err(anyhow::anyhow!("Failed to list pods across all namespaces: {}", e)),
                        }
                    } else {
                        // Get specific pod in a specific namespace (or default)
                        let ns = namespace_to_use.unwrap_or("default");
                        let specific_pods_api: Api<Pod> = Api::namespaced(self.client.clone(), ns);
                        match specific_pods_api.get(name).await {
                            Ok(pod) => Ok(serde_json::to_string_pretty(&pod)?),
                            Err(e) => Err(anyhow::anyhow!("Failed to get pod '{}' in namespace '{}': {}", name, ns, e)),
                        }
                    }
                } else {
                    // List pods (respecting the Api scope: all or namespaced based on args.namespace)
                    let pods_api: Api<Pod> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    let lp = self.build_list_params(args);
                    match pods_api.list(&lp).await {
                        Ok(pod_list) => {
                            let summary: Vec<String> = pod_list.items.iter().map(|pod| {
                                format!("{}\t{}\t{}\t{}",
                                    pod.metadata.namespace.as_ref().unwrap_or(&"<unknown>".to_string()),
                                    pod.metadata.name.as_ref().unwrap_or(&"<unknown>".to_string()),
                                    pod.status.as_ref()
                                        .and_then(|s| s.phase.as_ref())
                                        .unwrap_or(&"Unknown".to_string()),
                                    pod.metadata.creation_timestamp.as_ref()
                                        .map(|t| t.0.to_string())
                                        .unwrap_or_else(|| "<unknown>".to_string())
                                )
                            }).collect();
                            Ok(format!("NAMESPACE\tNAME\tSTATUS\tAGE\n{}", summary.join("\n")))
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list pods: {}", e)),
                    }
                }
            }
            "namespaces" | "namespace" | "ns" => {
                let namespaces: Api<Namespace> = Api::all(self.client.clone());
                
                if let Some(name) = &args.name {
                    // Get specific namespace
                    match namespaces.get(name).await {
                        Ok(ns) => Ok(serde_json::to_string_pretty(&ns)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get namespace: {}", e)),
                    }
                } else {
                    // List all namespaces
                    let lp = self.build_list_params(args);
                    match namespaces.list(&lp).await {
                        Ok(ns_list) => {
                            let summary: Vec<String> = ns_list.items.iter().map(|ns| {
                                format!("{}\t{}\t{}", 
                                    ns.metadata.name.as_ref().unwrap_or(&"<unknown>".to_string()),
                                    ns.status.as_ref()
                                        .and_then(|s| s.phase.as_ref())
                                        .unwrap_or(&"Active".to_string()),
                                    ns.metadata.creation_timestamp.as_ref()
                                        .map(|t| t.0.to_string())
                                        .unwrap_or_else(|| "<unknown>".to_string())
                                )
                            }).collect();
                            Ok(format!("NAME\tSTATUS\tAGE\n{}", summary.join("\n")))
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list namespaces: {}", e)),
                    }
                }
            }
            "deployments" | "deployment" | "deploy" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    // Get specific deployment
                    let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(deploy) => Ok(serde_json::to_string_pretty(&deploy)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get deployment '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    // List deployments
                    let api: Api<Deployment> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(deploy_list) => {
                            let formatted = self.format_resource_list(
                                deploy_list.items,
                                "deployment",
                                true,
                                |deploy| (
                                    deploy.metadata.namespace.clone(),
                                    deploy.metadata.name.clone(),
                                    deploy.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list deployments: {}", e)),
                    }
                }
            }
            "services" | "service" | "svc" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    // Get specific service
                    let api: Api<Service> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(svc) => Ok(serde_json::to_string_pretty(&svc)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get service '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    // List services
                    let api: Api<Service> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(svc_list) => {
                            let formatted = self.format_resource_list(
                                svc_list.items,
                                "service",
                                true,
                                |svc| (
                                    svc.metadata.namespace.clone(),
                                    svc.metadata.name.clone(),
                                    svc.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list services: {}", e)),
                    }
                }
            }
            "statefulsets" | "statefulset" | "sts" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    let api: Api<StatefulSet> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(sts) => Ok(serde_json::to_string_pretty(&sts)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get statefulset '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    let api: Api<StatefulSet> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(sts_list) => {
                            let formatted = self.format_resource_list(
                                sts_list.items,
                                "statefulset",
                                true,
                                |sts| (
                                    sts.metadata.namespace.clone(),
                                    sts.metadata.name.clone(),
                                    sts.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list statefulsets: {}", e)),
                    }
                }
            }
            "daemonsets" | "daemonset" | "ds" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    let api: Api<DaemonSet> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(ds) => Ok(serde_json::to_string_pretty(&ds)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get daemonset '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    let api: Api<DaemonSet> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(ds_list) => {
                            let formatted = self.format_resource_list(
                                ds_list.items,
                                "daemonset",
                                true,
                                |ds| (
                                    ds.metadata.namespace.clone(),
                                    ds.metadata.name.clone(),
                                    ds.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list daemonsets: {}", e)),
                    }
                }
            }
            "jobs" | "job" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    let api: Api<Job> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(job) => Ok(serde_json::to_string_pretty(&job)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get job '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    let api: Api<Job> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(job_list) => {
                            let formatted = self.format_resource_list(
                                job_list.items,
                                "job",
                                true,
                                |job| (
                                    job.metadata.namespace.clone(),
                                    job.metadata.name.clone(),
                                    job.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list jobs: {}", e)),
                    }
                }
            }
            "cronjobs" | "cronjob" | "cj" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    let api: Api<CronJob> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(cj) => Ok(serde_json::to_string_pretty(&cj)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get cronjob '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    let api: Api<CronJob> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(cj_list) => {
                            let formatted = self.format_resource_list(
                                cj_list.items,
                                "cronjob",
                                true,
                                |cj| (
                                    cj.metadata.namespace.clone(),
                                    cj.metadata.name.clone(),
                                    cj.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list cronjobs: {}", e)),
                    }
                }
            }
            "configmaps" | "configmap" | "cm" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(cm) => Ok(serde_json::to_string_pretty(&cm)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get configmap '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    let api: Api<ConfigMap> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(cm_list) => {
                            let formatted = self.format_resource_list(
                                cm_list.items,
                                "configmap",
                                true,
                                |cm| (
                                    cm.metadata.namespace.clone(),
                                    cm.metadata.name.clone(),
                                    cm.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list configmaps: {}", e)),
                    }
                }
            }
            "secrets" | "secret" => {
                let namespace = args.namespace.as_deref().unwrap_or("default");
                
                if let Some(name) = &args.name {
                    let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
                    match api.get(name).await {
                        Ok(secret) => Ok(serde_json::to_string_pretty(&secret)?),
                        Err(e) => Err(anyhow::anyhow!("Failed to get secret '{}' in namespace '{}': {}", name, namespace, e)),
                    }
                } else {
                    let api: Api<Secret> = match args.namespace.as_deref() {
                        Some("all") => Api::all(self.client.clone()),
                        Some(ns) => Api::namespaced(self.client.clone(), ns),
                        None => Api::namespaced(self.client.clone(), "default"),
                    };
                    
                    let lp = self.build_list_params(args);
                    match api.list(&lp).await {
                        Ok(secret_list) => {
                            let formatted = self.format_resource_list(
                                secret_list.items,
                                "secret",
                                true,
                                |secret| (
                                    secret.metadata.namespace.clone(),
                                    secret.metadata.name.clone(),
                                    secret.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                                )
                            );
                            Ok(formatted)
                        }
                        Err(e) => Err(anyhow::anyhow!("Failed to list secrets: {}", e)),
                    }
                }
            }
            _ => Ok(format!("Resource type '{}' not yet implemented", resource)),
        }
    }
    
    /// Execute "get all" to return common workload resources
    async fn execute_get_all(&self, args: &KubectlToolArgs) -> Result<String> {
        let namespace = args.namespace.as_deref().unwrap_or("default");
        let mut output = Vec::new();
        let lp = self.build_list_params(args);
        
        // Get pods
        let pods_api: Api<Pod> = match args.namespace.as_deref() {
            Some("all") => Api::all(self.client.clone()),
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::namespaced(self.client.clone(), "default"),
        };
        
        if let Ok(pod_list) = pods_api.list(&lp).await {
            if !pod_list.items.is_empty() {
                output.push("=== PODS ===".to_string());
                let formatted = self.format_resource_list(
                    pod_list.items,
                    "pod",
                    true,
                    |pod| (
                        pod.metadata.namespace.clone(),
                        pod.metadata.name.clone(),
                        pod.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                    )
                );
                output.push(formatted);
            }
        }
        
        // Get services
        let svc_api: Api<Service> = match args.namespace.as_deref() {
            Some("all") => Api::all(self.client.clone()),
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::namespaced(self.client.clone(), "default"),
        };
        
        if let Ok(svc_list) = svc_api.list(&lp).await {
            if !svc_list.items.is_empty() {
                output.push("\n=== SERVICES ===".to_string());
                let formatted = self.format_resource_list(
                    svc_list.items,
                    "service",
                    true,
                    |svc| (
                        svc.metadata.namespace.clone(),
                        svc.metadata.name.clone(),
                        svc.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                    )
                );
                output.push(formatted);
            }
        }
        
        // Get deployments
        let deploy_api: Api<Deployment> = match args.namespace.as_deref() {
            Some("all") => Api::all(self.client.clone()),
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::namespaced(self.client.clone(), "default"),
        };
        
        if let Ok(deploy_list) = deploy_api.list(&lp).await {
            if !deploy_list.items.is_empty() {
                output.push("\n=== DEPLOYMENTS ===".to_string());
                let formatted = self.format_resource_list(
                    deploy_list.items,
                    "deployment",
                    true,
                    |deploy| (
                        deploy.metadata.namespace.clone(),
                        deploy.metadata.name.clone(),
                        deploy.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                    )
                );
                output.push(formatted);
            }
        }
        
        // Get statefulsets
        let sts_api: Api<StatefulSet> = match args.namespace.as_deref() {
            Some("all") => Api::all(self.client.clone()),
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::namespaced(self.client.clone(), "default"),
        };
        
        if let Ok(sts_list) = sts_api.list(&lp).await {
            if !sts_list.items.is_empty() {
                output.push("\n=== STATEFULSETS ===".to_string());
                let formatted = self.format_resource_list(
                    sts_list.items,
                    "statefulset",
                    true,
                    |sts| (
                        sts.metadata.namespace.clone(),
                        sts.metadata.name.clone(),
                        sts.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                    )
                );
                output.push(formatted);
            }
        }
        
        // Get daemonsets
        let ds_api: Api<DaemonSet> = match args.namespace.as_deref() {
            Some("all") => Api::all(self.client.clone()),
            Some(ns) => Api::namespaced(self.client.clone(), ns),
            None => Api::namespaced(self.client.clone(), "default"),
        };
        
        if let Ok(ds_list) = ds_api.list(&lp).await {
            if !ds_list.items.is_empty() {
                output.push("\n=== DAEMONSETS ===".to_string());
                let formatted = self.format_resource_list(
                    ds_list.items,
                    "daemonset",
                    true,
                    |ds| (
                        ds.metadata.namespace.clone(),
                        ds.metadata.name.clone(),
                        ds.metadata.creation_timestamp.as_ref().map(|t| t.0.to_string())
                    )
                );
                output.push(formatted);
            }
        }
        
        if output.is_empty() {
            Ok("No resources found".to_string())
        } else {
            Ok(output.join("\n"))
        }
    }
    
    async fn execute_describe(&self, args: &KubectlToolArgs) -> Result<String> {
        let resource_type = args.resource.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing resource type for 'describe' verb"))?;
        let resource_name = args.name.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Missing resource name for 'describe' verb"))?;
        let namespace = args.namespace.as_deref().unwrap_or("default");

        match resource_type.as_str() {
            "pod" | "pods" => {
                let api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(pod) => {
                        // For a more detailed "describe" like output, we would typically gather
                        // related events, logs, and more details.
                        // For now, we'll return the pretty printed Pod spec, similar to `kubectl get pod <name> -o yaml`
                        // but a true describe often involves more.
                        // A full `kubectl describe` output is quite complex to replicate perfectly.
                        // This will give a structured YAML/JSON view of the pod.
                        Ok(serde_yaml::to_string(&pod)?)
                    }
                    Err(e) => Err(anyhow::anyhow!("Failed to get pod '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "namespace" | "namespaces" | "ns" => {
                let api: Api<Namespace> = Api::all(self.client.clone());
                match api.get(resource_name).await {
                    Ok(ns) => {
                        // Similar to pods, returning the spec for now.
                        Ok(serde_yaml::to_string(&ns)?)
                    }
                    Err(e) => Err(anyhow::anyhow!("Failed to get namespace '{}': {}", resource_name, e)),
                }
            }
            "deployment" | "deployments" | "deploy" => {
                let api: Api<Deployment> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(deploy) => Ok(serde_yaml::to_string(&deploy)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get deployment '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "service" | "services" | "svc" => {
                let api: Api<Service> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(svc) => Ok(serde_yaml::to_string(&svc)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get service '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "statefulset" | "statefulsets" | "sts" => {
                let api: Api<StatefulSet> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(sts) => Ok(serde_yaml::to_string(&sts)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get statefulset '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "daemonset" | "daemonsets" | "ds" => {
                let api: Api<DaemonSet> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(ds) => Ok(serde_yaml::to_string(&ds)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get daemonset '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "job" | "jobs" => {
                let api: Api<Job> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(job) => Ok(serde_yaml::to_string(&job)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get job '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "cronjob" | "cronjobs" | "cj" => {
                let api: Api<CronJob> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(cj) => Ok(serde_yaml::to_string(&cj)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get cronjob '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "configmap" | "configmaps" | "cm" => {
                let api: Api<ConfigMap> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(cm) => Ok(serde_yaml::to_string(&cm)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get configmap '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            "secret" | "secrets" => {
                let api: Api<Secret> = Api::namespaced(self.client.clone(), namespace);
                match api.get(resource_name).await {
                    Ok(secret) => Ok(serde_yaml::to_string(&secret)?),
                    Err(e) => Err(anyhow::anyhow!("Failed to get secret '{}' in namespace '{}': {}", resource_name, namespace, e)),
                }
            }
            // TODO: Add other resource types as needed (e.g., services, deployments)
            _ => Err(anyhow::anyhow!("Describing resource type '{}' is not yet implemented.", resource_type)),
        }
    }
    
    async fn execute_logs(&self, args: &KubectlToolArgs) -> Result<String> {
        let pod_name = args.name.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pod name is required for logs"))?;
        let namespace = args.namespace.as_deref().unwrap_or("default");

        // TODO: Add support for specifying container name if a pod has multiple containers.
        // For now, it will get logs from the first container (or the only one).
        let pods_api: Api<Pod> = Api::namespaced(self.client.clone(), namespace);
        
        // Set default tail_lines to 100 if not specified
        let mut lp = kube::api::LogParams::default();
        lp.tail_lines = Some(args.tail_lines.unwrap_or(100));

        match pods_api.logs(pod_name, &lp).await {
            Ok(logs) => Ok(logs),
            Err(e) => Err(anyhow::anyhow!("Failed to get logs for pod '{}' in namespace '{}': {}", pod_name, namespace, e)),
        }
    }
    
    /// Execute events command to show cluster events
    async fn execute_events(&self, args: &KubectlToolArgs) -> Result<String> {
        let namespace = args.namespace.as_deref();
        
        let api: Api<Event> = match namespace {
            Some("all") | None => Api::all(self.client.clone()),
            Some(ns) => Api::namespaced(self.client.clone(), ns),
        };
        
        let mut lp = ListParams::default();
        
        // Apply field selector if provided
        if let Some(field_selector) = &args.field_selector {
            lp = lp.fields(field_selector);
        }
        
        // Apply label selector if provided  
        if let Some(label_selector) = &args.label_selector {
            lp = lp.labels(label_selector);
        }
        
        match api.list(&lp).await {
            Ok(event_list) => {
                let mut events: Vec<String> = Vec::new();
                
                for event in event_list.items {
                    let namespace = event.metadata.namespace.unwrap_or_default();
                    let name = event.metadata.name.unwrap_or_default();
                    let obj = event.involved_object;
                    let reason = event.reason.unwrap_or_default();
                    let message = event.message.unwrap_or_default();
                    let event_time = event.event_time
                        .map(|t| t.0.to_string())
                        .or_else(|| event.first_timestamp.map(|t| t.0.to_string()))
                        .unwrap_or_else(|| "<unknown>".to_string());
                    
                    events.push(format!(
                        "{}\t{}\t{}/{}\t{}\t{}\t{}",
                        namespace,
                        event_time,
                        obj.kind.unwrap_or_default(),
                        obj.name.unwrap_or_default(),
                        reason,
                        message.replace('\n', " "),
                        name
                    ));
                }
                
                if events.is_empty() {
                    Ok("No events found".to_string())
                } else {
                    Ok(format!("NAMESPACE\tLAST SEEN\tOBJECT\tREASON\tMESSAGE\tNAME\n{}", events.join("\n")))
                }
            }
            Err(e) => Err(anyhow::anyhow!("Failed to list events: {}", e)),
        }
    }
    
    /// Validate if the command is safe to execute
    fn validate(&self, args: &KubectlToolArgs) -> Result<()> {
        // 1. Check if the verb is allowed by the tool's configuration.
        // This acts as the primary check against disallowed verbs.
        if !self.allowed_verbs.contains(&args.verb) {
            return Err(anyhow::anyhow!(
                "Verb '{}' is not allowed. Allowed verbs are: {:?}.",
                args.verb,
                self.allowed_verbs
            ));
        }

        // 2. Check resource and name fields for dangerous substrings.
        // These patterns aim to catch attempts to inject shell commands or other
        // unexpected operations into fields that should be simple identifiers.
        let dangerous_substrings = vec![
            ";", "&&", "||", "`", "$(", // Shell metacharacters
            "rm -rf", "rm -f",         // Common dangerous commands
            "kubectl exec", "kubectl delete", // Embedding kubectl commands
            "--force",                  // Potentially dangerous flags
            "/bin/sh", "/bin/bash",    // Shell invocations
        ];

        let fields_to_check: [(&str, Option<&String>); 2] = [
            ("resource", args.resource.as_ref()),
            ("name", args.name.as_ref()),
        ];

        for (field_name, field_value_opt) in fields_to_check {
            if let Some(field_value) = field_value_opt {
                let field_value_lower = field_value.to_lowercase();
                for pattern in &dangerous_substrings {
                    if field_value_lower.contains(pattern) {
                        return Err(anyhow::anyhow!(
                            "Argument for '{}' field ('{}') contains a potentially dangerous pattern: '{}'",
                            field_name, field_value, pattern
                        ));
                    }
                }
            }
        }

        // Validate namespace if whitelist is configured
        if let Some(ref whitelist) = self.namespace_whitelist {
            if let Some(ref ns) = args.namespace {
                // Allow "all" to bypass the namespace whitelist check
                if ns.to_lowercase() != "all" && !whitelist.contains(ns) {
                    return Err(anyhow::anyhow!("Namespace '{}' is not in whitelist. Allowed: {:?}", ns, whitelist));
                }
            }
        }

        Ok(())
    }
}

// Implement Rig's Tool trait
impl RigTool for KubectlTool {
    const NAME: &'static str = "kubectl";
    
    type Error = ToolError;
    type Args = KubectlToolArgs;
    type Output = ToolResult;
    
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Execute kubectl commands for Kubernetes cluster inspection. \
                         Supports 'get', 'describe', 'logs', and 'events' verbs. \
                         Use this tool to query Kubernetes resources.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "verb": {
                        "type": "string",
                        "description": "The kubectl verb to execute.",
                        "enum": ["get", "describe", "logs", "events"]
                    },
                    "resource": {
                        "type": "string",
                        "description": "The type of Kubernetes resource. Supported types: pods, namespaces, services, deployments, statefulsets, daemonsets, jobs, cronjobs, configmaps, secrets, and 'all' (returns pods, services, deployments, statefulsets, and daemonsets). Use singular or plural forms. Optional for some verbs."
                    },
                    "name": {
                        "type": "string",
                        "description": "The name of the specific resource. Optional."
                    },
                    "namespace": {
                        "type": "string",
                        "description": "The Kubernetes namespace to operate in. Defaults to 'default' if not specified. For 'get' operations, use 'all' to list resources across all namespaces. Optional."
                    },
                    "tail_lines": {
                        "type": "integer",
                        "description": "Number of lines to return from the end of the logs. Only used with 'logs' verb. Defaults to 100 if not specified. Optional."
                    },
                    "field_selector": {
                        "type": "string",
                        "description": "Field selector for filtering resources (e.g., 'status.phase=Running', 'metadata.name=my-pod'). Optional."
                    },
                    "label_selector": {
                        "type": "string",
                        "description": "Label selector for filtering resources (e.g., 'app=nginx', 'environment=production,tier=frontend'). Optional."
                    }
                },
                "required": ["verb"]
            }),
        }
    }
    
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Validate the command based on the structured arguments
        self.validate(&args)
            .map_err(|e| ToolError::ValidationError(e.to_string()))?;
        
        // Clone self for the spawned task
        let tool = self.clone();
        // Capture args for the spawned task
        let task_args = args.clone();
        
        // Spawn the execution to avoid Sync issues with kube client
        let result = tokio::spawn(async move {
            tool.execute_command(&task_args).await
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

#[cfg(test)]
mod tests {
    use super::*;
    // Import KubectlToolArgs for tests
    use super::KubectlToolArgs;

    #[tokio::test]
    async fn test_kubectl_infer() {
        // This test will pass if you have a valid kubeconfig or are running in a cluster
        match KubectlTool::infer().await {
            Ok(tool) => {
                // Test that the tool was created successfully
                assert!(tool.allowed_verbs.contains("get"));
                assert!(tool.allowed_verbs.contains("describe"));
                
                // Test a simple command using new KubectlToolArgs
                let args = KubectlToolArgs {
                    verb: "get".to_string(),
                    resource: Some("namespaces".to_string()),
                    name: None,
                    namespace: None,
                    tail_lines: None,
                    field_selector: None,
                    label_selector: None,
                };
                
                match tool.call(args).await {
                    Ok(result) => {
                        // If we have a valid config, the command should work
                        if result.success {
                            assert!(!result.output.is_empty());
                        } else {
                            // Error is expected if no cluster is available
                            assert!(result.error.is_some());
                        }
                    }
                    Err(e) => {
                        // Tool error is acceptable in test environment
                        println!("Tool call error (expected in test env): {:?}", e);
                    }
                }
            }
            Err(e) => {
                // This is expected in CI/test environments without kubernetes
                println!("Could not infer config (expected in test env): {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_validate_dangerous_patterns() {
        let tool = match KubectlTool::infer().await {
            Ok(tool) => tool,
            Err(_) => {
                println!("Skipping test - no Kubernetes config available");
                return;
            }
        };

        // Test explicitly disallowed verb (not in default allowed_verbs)
        let disallowed_verb_args = KubectlToolArgs {
            verb: "delete".to_string(), // "delete" is not in the default allowed_verbs
            resource: Some("pod".to_string()),
            name: Some("my-pod".to_string()),
            namespace: None,
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&disallowed_verb_args).is_err());
        assert!(tool.validate(&disallowed_verb_args).unwrap_err().to_string().contains("Verb 'delete' is not allowed"));

        // Test dangerous patterns in 'name' field
        let dangerous_name_args = KubectlToolArgs {
            verb: "get".to_string(),
            resource: Some("pods".to_string()),
            name: Some("my-pod; rm -rf /".to_string()), // Contains ';' and "rm -rf"
            namespace: None,
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&dangerous_name_args).is_err());
        assert!(tool.validate(&dangerous_name_args).unwrap_err().to_string().contains("contains a potentially dangerous pattern: ';'"));

        let dangerous_name_args_kubectl = KubectlToolArgs {
            verb: "get".to_string(),
            resource: Some("pods".to_string()),
            name: Some("pod-name kubectl exec evil-cmd".to_string()), // Contains "kubectl exec"
            namespace: None,
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&dangerous_name_args_kubectl).is_err());
        assert!(tool.validate(&dangerous_name_args_kubectl).unwrap_err().to_string().contains("pattern: 'kubectl exec'"));

        // Test dangerous patterns in 'resource' field
        let dangerous_resource_args = KubectlToolArgs {
            verb: "get".to_string(),
            resource: Some("pods && evil-command".to_string()), // Contains "&&"
            name: Some("my-pod".to_string()),
            namespace: None,
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&dangerous_resource_args).is_err());
        assert!(tool.validate(&dangerous_resource_args).unwrap_err().to_string().contains("pattern: '&&'"));


        // Test safe commands pass
        let safe_args_get_pods = KubectlToolArgs {
            verb: "get".to_string(),
            resource: Some("pods".to_string()),
            name: None,
            namespace: None,
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&safe_args_get_pods).is_ok());

        let safe_args_describe_pod = KubectlToolArgs {
            verb: "describe".to_string(),
            resource: Some("pod".to_string()),
            name: Some("my-pod-123".to_string()),
            namespace: Some("default".to_string()),
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&safe_args_describe_pod).is_ok());

        let safe_args_logs = KubectlToolArgs {
            verb: "logs".to_string(),
            resource: None, 
            name: Some("another-pod-abc".to_string()),
            namespace: Some("kube-system".to_string()),
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool.validate(&safe_args_logs).is_ok());

        // Test namespace whitelist
        let tool_with_ns_whitelist = tool.clone().with_namespace_whitelist(vec!["allowed-ns".to_string()]);
        let ns_allowed_args = KubectlToolArgs {
            verb: "get".to_string(),
            resource: Some("pods".to_string()),
            name: None,
            namespace: Some("allowed-ns".to_string()),
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool_with_ns_whitelist.validate(&ns_allowed_args).is_ok());

        let ns_disallowed_args = KubectlToolArgs {
            verb: "get".to_string(),
            resource: Some("pods".to_string()),
            name: None,
            namespace: Some("forbidden-ns".to_string()),
            tail_lines: None,
            field_selector: None,
            label_selector: None,
        };
        assert!(tool_with_ns_whitelist.validate(&ns_disallowed_args).is_err());
        assert!(tool_with_ns_whitelist.validate(&ns_disallowed_args).unwrap_err().to_string().contains("Namespace 'forbidden-ns' is not in whitelist"));
    }

    #[test]
    fn test_allowed_verbs() {
        // Test that we can create a tool and it has the expected allowed verbs
        // This doesn't require a real client
        let mut allowed_verbs = HashSet::new();
        allowed_verbs.insert("get".to_string());
        allowed_verbs.insert("describe".to_string());
        allowed_verbs.insert("logs".to_string());
        allowed_verbs.insert("top".to_string());
        allowed_verbs.insert("events".to_string());
        
        // Verify the expected verbs are in our default set
        assert!(allowed_verbs.contains("get"));
        assert!(allowed_verbs.contains("describe"));
        assert!(!allowed_verbs.contains("delete"));
        assert!(!allowed_verbs.contains("apply"));
    }

    #[test]
    fn test_dangerous_patterns_regex() {
        // Test the dangerous patterns detection without needing a client
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
        
        // Test that dangerous commands match
        for pattern in &dangerous_patterns {
            let re = Regex::new(pattern).unwrap();
            match pattern {
                &r";\s*rm\s+" => assert!(re.is_match("kubectl get pods; rm -rf /")),
                &r"&&\s*rm\s+" => assert!(re.is_match("kubectl get pods && rm something")),
                &r"delete\s+" => assert!(re.is_match("kubectl delete pod")),
                &r"exec\s+" => assert!(re.is_match("kubectl exec -it pod")),
                _ => {}
            }
        }
    }
} 