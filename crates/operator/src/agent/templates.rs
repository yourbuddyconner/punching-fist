//! Investigation Templates
//! 
//! Pre-defined investigation templates for common alert types.

use std::collections::HashMap;

/// Get investigation template for a specific alert type
pub fn get_investigation_template(alert_name: &str) -> Option<InvestigationTemplate> {
    let templates = create_templates();
    templates.get(alert_name).cloned()
}

/// Investigation template for a specific alert type
#[derive(Debug, Clone)]
pub struct InvestigationTemplate {
    /// Name of the alert type
    pub alert_name: String,
    /// Description of what this alert means
    pub description: String,
    /// Initial investigation steps
    pub initial_steps: Vec<InvestigationStep>,
    /// Success criteria for resolution
    pub success_criteria: String,
}

#[derive(Debug, Clone)]
pub struct InvestigationStep {
    pub command_template: String,
    pub description: String,
}

fn create_templates() -> HashMap<String, InvestigationTemplate> {
    let mut templates = HashMap::new();
    
    // PodCrashLooping template
    templates.insert("PodCrashLooping".to_string(), InvestigationTemplate {
        alert_name: "PodCrashLooping".to_string(),
        description: "Pod is repeatedly crashing and restarting".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Get pod details and recent events".to_string(),
                command_template: "kubectl describe pod {{ pod_name }} -n {{ namespace }}".to_string(),
            },
            InvestigationStep {
                description: "Check pod logs from previous crash".to_string(),
                command_template: "kubectl logs {{ pod_name }} -n {{ namespace }} --previous --tail=100".to_string(),
            },
            InvestigationStep {
                description: "Check memory usage".to_string(),
                command_template: "container_memory_usage_bytes{pod=\"{{ pod_name }}\",namespace=\"{{ namespace }}\"}".to_string(),
            },
        ],
        success_criteria: "Identify root cause of crashes (OOM, configuration error, dependency failure)".to_string(),
    });
    
    // HighCPUUsage template
    templates.insert("HighCPUUsage".to_string(), InvestigationTemplate {
        alert_name: "HighCPUUsage".to_string(),
        description: "Service is experiencing high CPU usage".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check CPU usage metrics".to_string(),
                command_template: "rate(container_cpu_usage_seconds_total{pod=~\"{{ service }}.*\",namespace=\"{{ namespace }}\"}[5m])".to_string(),
            },
            InvestigationStep {
                description: "Get process information inside container".to_string(),
                command_template: "kubectl exec {{ pod_name }} -n {{ namespace }} -- top -b -n 1".to_string(),
            },
            InvestigationStep {
                description: "Check recent deployments".to_string(),
                command_template: "kubectl rollout history deployment {{ deployment }} -n {{ namespace }}".to_string(),
            },
        ],
        success_criteria: "Determine if high CPU is due to legitimate load or inefficient code".to_string(),
    });
    
    // ServiceUnavailable template
    templates.insert("ServiceUnavailable".to_string(), InvestigationTemplate {
        alert_name: "ServiceUnavailable".to_string(),
        description: "Service is not responding or has no healthy endpoints".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check service endpoints".to_string(),
                command_template: "kubectl get endpoints {{ service }} -n {{ namespace }}".to_string(),
            },
            InvestigationStep {
                description: "Check pod status".to_string(),
                command_template: "kubectl get pods -l app={{ service }} -n {{ namespace }}".to_string(),
            },
            InvestigationStep {
                description: "Test service connectivity".to_string(),
                command_template: "curl -s -o /dev/null -w '%{http_code}' http://{{ service }}.{{ namespace }}.svc.cluster.local:{{ port }}/health".to_string(),
            },
            InvestigationStep {
                description: "Check recent logs".to_string(),
                command_template: "kubectl logs -l app={{ service }} -n {{ namespace }} --tail=50 --since=5m".to_string(),
            },
        ],
        success_criteria: "Service is responding and has healthy endpoints".to_string(),
    });
    
    // HighMemoryUsage template
    templates.insert("HighMemoryUsage".to_string(), InvestigationTemplate {
        alert_name: "HighMemoryUsage".to_string(),
        description: "Pod is using high amount of memory".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check current memory usage".to_string(),
                command_template: "container_memory_usage_bytes{pod=\"{{ pod_name }}\",namespace=\"{{ namespace }}\"}".to_string(),
            },
            InvestigationStep {
                description: "Get memory limits".to_string(),
                command_template: "kubectl get pod {{ pod_name }} -n {{ namespace }} -o jsonpath='{.spec.containers[*].resources.limits.memory}'".to_string(),
            },
            InvestigationStep {
                description: "Check for memory leaks in logs".to_string(),
                command_template: "kubectl logs {{ pod_name }} -n {{ namespace }} --tail=100 | grep -i 'memory\\|heap\\|oom'".to_string(),
            },
        ],
        success_criteria: "Determine if memory usage is expected or indicates a memory leak".to_string(),
    });
    
    templates
}

/// System prompt for investigation agents
pub const INVESTIGATION_SYSTEM_PROMPT: &str = r#"You are an expert Kubernetes SRE tasked with investigating alerts and issues.

Your approach should be:
1. Systematic - follow a logical investigation path
2. Evidence-based - support conclusions with data
3. Action-oriented - provide clear next steps
4. Risk-aware - consider the impact of any recommended actions

When investigating:
- Start with understanding the current state
- Gather relevant metrics and logs
- Look for recent changes
- Consider the broader system context
- Identify root causes, not just symptoms

For each investigation, provide:
- A clear summary of findings
- Root cause analysis
- Specific recommendations
- Risk assessment for any actions
"#;

/// Build investigation prompt based on alert
pub fn build_investigation_prompt(alert_name: &str, context: &serde_json::Value) -> String {
    let mut prompt = String::from(INVESTIGATION_SYSTEM_PROMPT);
    prompt.push_str("\n\n");
    
    // Add alert-specific context
    prompt.push_str(&format!("Alert: {}\n", alert_name));
    
    if let Some(severity) = context.get("severity").and_then(|v| v.as_str()) {
        prompt.push_str(&format!("Severity: {}\n", severity));
    }
    
    if let Some(namespace) = context.get("namespace").and_then(|v| v.as_str()) {
        prompt.push_str(&format!("Namespace: {}\n", namespace));
    }
    
    // Add template guidance if available
    if let Some(template) = get_investigation_template(alert_name) {
        prompt.push_str(&format!("\nAlert Description: {}\n", template.description));
        prompt.push_str(&template.description);
        
        prompt.push_str("\n\nSuggested investigation steps:\n");
        for (i, step) in template.initial_steps.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, step.description));
        }
        
        prompt.push_str(&format!("\nSuccess criteria: {}\n", template.success_criteria));
    }
    
    prompt.push_str("\n\nPlease investigate this issue thoroughly and provide actionable recommendations.");
    
    prompt
} 