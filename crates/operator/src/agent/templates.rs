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
    pub name: String,
    pub description: String,
    pub initial_steps: Vec<InvestigationStep>,
    pub success_criteria: String,
    pub escalation_threshold: String,
}

#[derive(Debug, Clone)]
pub struct InvestigationStep {
    pub description: String,
    pub tool: String,
    pub command_template: String,
    pub expected_output: String,
}

fn create_templates() -> HashMap<String, InvestigationTemplate> {
    let mut templates = HashMap::new();
    
    // Pod crash loop investigation
    templates.insert("PodCrashLooping".to_string(), InvestigationTemplate {
        name: "Pod Crash Loop Investigation".to_string(),
        description: "Investigate why a pod is crash looping".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check pod status and events".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl describe pod {{pod_name}} -n {{namespace}}".to_string(),
                expected_output: "Exit code, restart count, and recent events".to_string(),
            },
            InvestigationStep {
                description: "Get recent logs from crashed container".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl logs {{pod_name}} -n {{namespace}} --previous --tail=100".to_string(),
                expected_output: "Error messages or stack traces".to_string(),
            },
            InvestigationStep {
                description: "Check resource usage if OOMKilled".to_string(),
                tool: "promql".to_string(),
                command_template: "container_memory_usage_bytes{pod=\"{{pod_name}}\",namespace=\"{{namespace}}\"}".to_string(),
                expected_output: "Memory usage metrics".to_string(),
            },
        ],
        success_criteria: "Identify root cause of crash (OOM, config error, dependency failure)".to_string(),
        escalation_threshold: "Unable to determine cause after checking logs and resources".to_string(),
    });
    
    // High CPU usage investigation
    templates.insert("HighCPUUsage".to_string(), InvestigationTemplate {
        name: "High CPU Usage Investigation".to_string(),
        description: "Investigate high CPU usage in a service".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check current CPU usage".to_string(),
                tool: "promql".to_string(),
                command_template: "rate(container_cpu_usage_seconds_total{pod=~\"{{service}}.*\",namespace=\"{{namespace}}\"}[5m])".to_string(),
                expected_output: "Current CPU usage rate".to_string(),
            },
            InvestigationStep {
                description: "Get top processes in pod".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl exec {{pod_name}} -n {{namespace}} -- top -b -n 1".to_string(),
                expected_output: "Process list with CPU usage".to_string(),
            },
            InvestigationStep {
                description: "Check for recent deployments".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl rollout history deployment {{deployment}} -n {{namespace}}".to_string(),
                expected_output: "Recent deployment history".to_string(),
            },
        ],
        success_criteria: "Identify process or change causing high CPU usage".to_string(),
        escalation_threshold: "CPU usage is legitimate but requires scaling".to_string(),
    });
    
    // Service unavailable investigation
    templates.insert("ServiceUnavailable".to_string(), InvestigationTemplate {
        name: "Service Unavailable Investigation".to_string(),
        description: "Investigate why a service is unavailable".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check service endpoints".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl get endpoints {{service}} -n {{namespace}}".to_string(),
                expected_output: "List of healthy endpoints".to_string(),
            },
            InvestigationStep {
                description: "Check pod status for service".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl get pods -l app={{service}} -n {{namespace}}".to_string(),
                expected_output: "Pod status and readiness".to_string(),
            },
            InvestigationStep {
                description: "Test service connectivity".to_string(),
                tool: "curl".to_string(),
                command_template: "curl -s -o /dev/null -w '%{http_code}' http://{{service}}.{{namespace}}.svc.cluster.local:{{port}}/health".to_string(),
                expected_output: "HTTP status code".to_string(),
            },
            InvestigationStep {
                description: "Check recent errors in logs".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl logs -l app={{service}} -n {{namespace}} --tail=50 --since=5m".to_string(),
                expected_output: "Recent error messages".to_string(),
            },
        ],
        success_criteria: "Service is reachable or root cause identified".to_string(),
        escalation_threshold: "Infrastructure issue beyond application scope".to_string(),
    });
    
    // High memory usage investigation
    templates.insert("HighMemoryUsage".to_string(), InvestigationTemplate {
        name: "High Memory Usage Investigation".to_string(),
        description: "Investigate high memory usage in a pod".to_string(),
        initial_steps: vec![
            InvestigationStep {
                description: "Check current memory usage".to_string(),
                tool: "promql".to_string(),
                command_template: "container_memory_usage_bytes{pod=\"{{pod_name}}\",namespace=\"{{namespace}}\"}".to_string(),
                expected_output: "Current memory usage in bytes".to_string(),
            },
            InvestigationStep {
                description: "Check memory limits".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl get pod {{pod_name}} -n {{namespace}} -o jsonpath='{.spec.containers[*].resources.limits.memory}'".to_string(),
                expected_output: "Memory limit configuration".to_string(),
            },
            InvestigationStep {
                description: "Look for memory leaks in logs".to_string(),
                tool: "kubectl".to_string(),
                command_template: "kubectl logs {{pod_name}} -n {{namespace}} --tail=100 | grep -i 'memory\\|heap\\|oom'".to_string(),
                expected_output: "Memory-related log entries".to_string(),
            },
        ],
        success_criteria: "Identify if memory usage is normal or indicates a leak".to_string(),
        escalation_threshold: "Memory leak confirmed, requires application fix".to_string(),
    });
    
    templates
}

/// System prompt for investigation
pub const INVESTIGATION_SYSTEM_PROMPT: &str = r#"
You are an expert Kubernetes and infrastructure engineer investigating production alerts.
Your goal is to quickly identify the root cause of issues and provide actionable recommendations.

When investigating:
1. Start with the most likely causes based on the alert type
2. Gather evidence systematically using the available tools
3. Correlate findings across different data sources
4. Consider recent changes that might have triggered the issue
5. Provide clear, actionable recommendations

Always structure your investigation:
- Initial hypothesis based on the alert
- Evidence gathering using tools
- Analysis of findings
- Root cause determination
- Recommendations for resolution

Be concise but thorough. Focus on facts and evidence.
"#;

/// Get prompt for specific investigation
pub fn get_investigation_prompt(alert_name: &str, context: &HashMap<String, String>) -> String {
    let mut prompt = String::new();
    
    // Add context
    prompt.push_str("I need to investigate this alert:\n\n");
    prompt.push_str(&format!("Alert: {}\n", alert_name));
    
    for (key, value) in context {
        prompt.push_str(&format!("{}: {}\n", key, value));
    }
    
    // Add template guidance if available
    if let Some(template) = get_investigation_template(alert_name) {
        prompt.push_str("\n\nSuggested investigation approach:\n");
        prompt.push_str(&template.description);
        prompt.push_str("\n\nRecommended initial steps:\n");
        
        for (i, step) in template.initial_steps.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, step.description));
        }
        
        prompt.push_str(&format!("\nSuccess criteria: {}\n", template.success_criteria));
    }
    
    prompt.push_str("\n\nPlease investigate this issue and provide your findings.");
    
    prompt
} 