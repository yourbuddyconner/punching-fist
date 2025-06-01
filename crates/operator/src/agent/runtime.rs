//! Agent Runtime
//! 
//! Core agent execution engine that uses Rig's agent system with integrated tools.

use super::{
    provider::{LLMProvider, LLMConfig},
    result::{AgentResult, ActionTaken, Finding, FindingSeverity, Recommendation, RiskLevel},
    safety::{SafetyValidator, SafetyConfig},
    templates,
    tools::{KubectlTool, PromQLTool, CurlTool, ScriptTool},
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, error, debug};
use serde_json;
use rig::{
    agent::Agent,
    completion::Chat,
    providers::{anthropic, openai},
};
use regex::Regex;
use chrono::Utc;
use kube::Client as K8sClient;

/// Enum to store different tool types
#[derive(Clone)]
pub enum ToolType {
    Kubectl(KubectlTool),
    PromQL(PromQLTool),
    Curl(CurlTool),
    Script(ScriptTool),
}

// Implement From traits for each tool type
impl From<KubectlTool> for ToolType {
    fn from(tool: KubectlTool) -> Self {
        ToolType::Kubectl(tool)
    }
}

impl From<PromQLTool> for ToolType {
    fn from(tool: PromQLTool) -> Self {
        ToolType::PromQL(tool)
    }
}

impl From<CurlTool> for ToolType {
    fn from(tool: CurlTool) -> Self {
        ToolType::Curl(tool)
    }
}

impl From<ScriptTool> for ToolType {
    fn from(tool: ScriptTool) -> Self {
        ToolType::Script(tool)
    }
}

/// Agent runtime for executing investigations
pub struct AgentRuntime {
    llm_config: LLMConfig,
    safety_validator: SafetyValidator,
    max_iterations: u32,
    timeout: std::time::Duration,
    k8s_client: Option<K8sClient>,
    prometheus_endpoint: String,
    tools: HashMap<String, ToolType>,
}

impl AgentRuntime {
    /// Create a new agent runtime
    pub fn new(llm_config: LLMConfig) -> Result<Self> {
        let safety_validator = SafetyValidator::new(SafetyConfig::default());
        
        // Extract values before moving llm_config
        let max_iterations = llm_config.max_tokens.unwrap_or(15);
        let timeout_seconds = llm_config.timeout_seconds.unwrap_or(300);
        
        Ok(Self {
            llm_config,
            safety_validator,
            max_iterations,
            timeout: std::time::Duration::from_secs(timeout_seconds),
            k8s_client: None,
            prometheus_endpoint: "http://prometheus:9090".to_string(),
            tools: HashMap::new(),
        })
    }
    
    /// Set Kubernetes client
    pub fn with_k8s_client(mut self, client: K8sClient) -> Self {
        self.k8s_client = Some(client);
        self
    }
    
    /// Set Prometheus endpoint
    pub fn with_prometheus_endpoint(mut self, endpoint: String) -> Self {
        self.prometheus_endpoint = endpoint;
        self
    }
    
    /// Add a tool to the runtime
    pub fn add_tool<T>(&mut self, name: String, tool: T) 
    where 
        T: Into<ToolType>
    {
        self.tools.insert(name, tool.into());
    }
    
    /// Build a Rig agent with tools for a specific provider
    async fn build_and_chat(&self, prompt: &str) -> Result<String> {
        match self.llm_config.provider.as_str() {
            "anthropic" | "claude" => {
                let client = if let Some(key) = &self.llm_config.api_key {
                    anthropic::Client::new(
                        key,
                        "https://api.anthropic.com",
                        None,
                        anthropic::ANTHROPIC_VERSION_LATEST,
                    )
                } else {
                    anthropic::Client::from_env()
                };
                
                let mut builder = client.agent(&self.llm_config.model);
                
                // Add stored tools to the builder
                for (name, tool) in &self.tools {
                    match tool {
                        ToolType::Kubectl(kubectl_tool) => {
                            builder = builder.tool(kubectl_tool.clone());
                        }
                        ToolType::PromQL(promql_tool) => {
                            builder = builder.tool(promql_tool.clone());
                        }
                        ToolType::Curl(curl_tool) => {
                            builder = builder.tool(curl_tool.clone());
                        }
                        ToolType::Script(script_tool) => {
                            builder = builder.tool(script_tool.clone());
                        }
                    }
                    debug!("Added tool: {}", name);
                }
                
                // If no tools were explicitly added but k8s client is available, add default tools
                if self.tools.is_empty() && self.k8s_client.is_some() {
                    if let Some(k8s_client) = &self.k8s_client {
                        builder = builder
                            .tool(KubectlTool::new(k8s_client.clone()))
                            .tool(PromQLTool::new(self.prometheus_endpoint.clone()))
                            .tool(CurlTool::new())
                            .tool(ScriptTool::new());
                    }
                }
                
                let agent = builder.build();
                agent.chat(prompt, vec![])
                    .await
                    .map_err(|e| anyhow::anyhow!("Anthropic chat failed: {:?}", e))
            }
            "openai" => {
                let client = if let Some(key) = &self.llm_config.api_key {
                    openai::Client::new(key)
                } else {
                    openai::Client::from_env()
                };
                
                let mut builder = client.agent(&self.llm_config.model);
                
                // Add stored tools to the builder
                for (name, tool) in &self.tools {
                    match tool {
                        ToolType::Kubectl(kubectl_tool) => {
                            builder = builder.tool(kubectl_tool.clone());
                        }
                        ToolType::PromQL(promql_tool) => {
                            builder = builder.tool(promql_tool.clone());
                        }
                        ToolType::Curl(curl_tool) => {
                            builder = builder.tool(curl_tool.clone());
                        }
                        ToolType::Script(script_tool) => {
                            builder = builder.tool(script_tool.clone());
                        }
                    }
                    debug!("Added tool: {}", name);
                }
                
                // If no tools were explicitly added but k8s client is available, add default tools
                if self.tools.is_empty() && self.k8s_client.is_some() {
                    if let Some(k8s_client) = &self.k8s_client {
                        builder = builder
                            .tool(KubectlTool::new(k8s_client.clone()))
                            .tool(PromQLTool::new(self.prometheus_endpoint.clone()))
                            .tool(CurlTool::new())
                            .tool(ScriptTool::new());
                    }
                }
                
                let agent = builder.build();
                agent.chat(prompt, vec![])
                    .await
                    .map_err(|e| anyhow::anyhow!("OpenAI chat failed: {:?}", e))
            }
            _ => {
                // For mock provider, return a mock response
                Ok(self.mock_investigation_response(prompt))
            }
        }
    }
    
    /// Execute an investigation using Rig's agent system
    pub async fn investigate(
        &self,
        goal: &str,
        context: HashMap<String, String>,
    ) -> Result<AgentResult> {
        info!("Starting agent investigation");
        debug!("Goal: {}", goal);
        debug!("Context: {:?}", context);
        
        let mut result = AgentResult::new(
            "Investigation in progress...".to_string()
        );
        
        // Get alert name from context for template lookup
        let alert_name = context.get("alert_name").cloned().unwrap_or_default();
        
        // Build investigation prompt
        let prompt = templates::get_investigation_prompt(&alert_name, &context);
        
        // Build the investigation prompt
        let investigation_prompt = format!(
            "{}\n\nGoal: {}\n\n\
            Investigate this issue step by step. Use the available tools to gather evidence. \
            After investigation, provide:\n\
            1. Root cause analysis\n\
            2. Key findings\n\
            3. Recommendations\n\
            4. Whether this can be auto-fixed\n\n\
            Structure your final response with clear sections:\n\
            ROOT CAUSE: <explanation>\n\
            FINDINGS:\n- finding 1\n- finding 2\n\
            RECOMMENDATIONS:\n- recommendation 1\n- recommendation 2\n\
            AUTO-FIX: <yes/no and command if applicable>",
            prompt, goal
        );
        
        // Use the provider-specific agent
        let response = self.build_and_chat(&investigation_prompt).await?;
        
        debug!("Agent response: {}", response);
        
        // Parse the structured response
        self.parse_final_analysis(&response, &mut result)?;
        
        // Note: With Rig's agent system, tool calls are handled automatically
        // We don't have access to the individual tool calls made, so we can't populate actions_taken
        // This is a limitation of using the high-level agent interface
        
        // Get confidence score from LLM
        result.confidence = (self.get_confidence_score(&result).await.unwrap_or(60.0) / 100.0) as f32;
        
        Ok(result)
    }
    
    /// Mock investigation response for testing
    fn mock_investigation_response(&self, prompt: &str) -> String {
        if prompt.contains("PodCrashLooping") {
            "ROOT CAUSE: The pod is experiencing an OutOfMemoryError due to insufficient memory limits.\n\n\
            FINDINGS:\n\
            - Pod shows exit code 137 (OOMKilled)\n\
            - Java heap space errors detected in logs\n\
            - Memory usage consistently at limit (512MB)\n\n\
            RECOMMENDATIONS:\n\
            - Increase memory limit to 1GB\n\
            - Add JVM heap size configuration\n\
            - Monitor memory usage after changes\n\n\
            AUTO-FIX: yes\nkubectl patch deployment my-app -n default -p '{\"spec\":{\"template\":{\"spec\":{\"containers\":[{\"name\":\"app\",\"resources\":{\"limits\":{\"memory\":\"1Gi\"}}}]}}}}'".to_string()
        } else if prompt.contains("HighCPUUsage") {
            "ROOT CAUSE: Service experiencing high legitimate traffic load.\n\n\
            FINDINGS:\n\
            - CPU usage at 95% across all pods\n\
            - Request rate increased 3x in past hour\n\
            - No signs of inefficient code or runaway processes\n\n\
            RECOMMENDATIONS:\n\
            - Scale deployment to 5 replicas\n\
            - Enable horizontal pod autoscaling\n\
            - Review and optimize high-CPU code paths\n\n\
            AUTO-FIX: yes\nkubectl scale deployment api-gateway -n production --replicas=5".to_string()
        } else {
            "ROOT CAUSE: Unable to determine specific root cause without more information.\n\n\
            FINDINGS:\n\
            - Alert indicates potential issue\n\
            - Manual investigation required\n\n\
            RECOMMENDATIONS:\n\
            - Check application logs\n\
            - Review recent deployments\n\
            - Monitor system metrics\n\n\
            AUTO-FIX: no".to_string()
        }
    }
    
    /// Ask the LLM to score its confidence in the investigation
    async fn get_confidence_score(&self, result: &AgentResult) -> Result<f64> {
        let confidence_prompt = format!(
            "Based on the following investigation results, provide a confidence score from 1-100.\n\n\
            Summary: {}\n\
            Root Cause: {}\n\
            Findings: {} total\n\
            Recommendations: {} total\n\
            Can Auto-Fix: {}\n\n\
            Respond with ONLY a number between 1 and 100.\n\
            CONFIDENCE SCORE:",
            result.summary,
            result.root_cause.as_ref().unwrap_or(&"Not identified".to_string()),
            result.findings.len(),
            result.recommendations.len(),
            result.can_auto_fix
        );
        
        let response = match self.llm_config.provider.as_str() {
            "anthropic" | "claude" | "openai" => {
                self.build_and_chat(&confidence_prompt).await?
            }
            _ => "75".to_string(), // Default for mock provider
        };
        
        self.parse_confidence_from_response(&response)
    }
    
    /// Parse confidence score from LLM response
    fn parse_confidence_from_response(&self, response: &str) -> Result<f64> {
        // Try to find a number in the response
        let number_regex = Regex::new(r"(\d+(?:\.\d+)?)")?;
        
        if let Some(cap) = number_regex.captures(response.trim()) {
            if let Some(num_str) = cap.get(1) {
                if let Ok(score) = num_str.as_str().parse::<f64>() {
                    // Ensure it's in valid range
                    if score >= 1.0 && score <= 100.0 {
                        return Ok(score);
                    }
                }
            }
        }
        
        // If we can't parse a valid number, return a default
        warn!("Could not parse confidence score from response: {}", response);
        Ok(60.0) // Default confidence
    }
    
    /// Parse final analysis from LLM response
    fn parse_final_analysis(&self, response: &str, result: &mut AgentResult) -> Result<()> {
        // Extract root cause
        if let Some(root_cause) = self.extract_section(response, &["ROOT CAUSE:", "root cause:", "Root Cause:"]) {
            result.root_cause = Some(root_cause);
        }
        
        // Extract findings
        if let Some(findings_text) = self.extract_section(response, &["FINDINGS:", "findings:", "Findings:"]) {
            for line in findings_text.lines() {
                let line = line.trim();
                if !line.is_empty() && (line.starts_with('-') || line.starts_with('•')) {
                    let finding_text = line.trim_start_matches('-').trim_start_matches('•').trim();
                    result.add_finding(Finding {
                        category: "Investigation".to_string(),
                        description: finding_text.to_string(),
                        severity: FindingSeverity::Medium,
                        evidence: HashMap::new(),
                    });
                }
            }
        }
        
        // Extract recommendations
        if let Some(recommendations_text) = self.extract_section(response, &["RECOMMENDATIONS:", "recommendations:", "Recommendations:"]) {
            let mut priority = 1;
            for line in recommendations_text.lines() {
                let line = line.trim();
                if !line.is_empty() && (line.starts_with('-') || line.starts_with('•') || line.starts_with(char::is_numeric)) {
                    let rec_text = line
                        .trim_start_matches(char::is_numeric)
                        .trim_start_matches('.')
                        .trim_start_matches('-')
                        .trim_start_matches('•')
                        .trim();
                    result.add_recommendation(Recommendation {
                        priority,
                        action: rec_text.to_string(),
                        rationale: "Recommended by AI investigation".to_string(),
                        risk_level: RiskLevel::Low,
                        requires_approval: true,
                    });
                    priority += 1;
                }
            }
        }
        
        // Extract auto-fix capability
        if let Some(autofix_text) = self.extract_section(response, &["AUTO-FIX:", "auto-fix:", "Auto-Fix:"]) {
            let autofix_lower = autofix_text.to_lowercase();
            result.can_auto_fix = autofix_lower.contains("yes") || autofix_lower.contains("true");
            
            // Extract fix command if present
            if result.can_auto_fix {
                // Look for kubectl commands in the auto-fix section
                let kubectl_regex = Regex::new(r"kubectl\s+[^\n]+")?;
                if let Some(match_) = kubectl_regex.find(&autofix_text) {
                    result.fix_command = Some(match_.as_str().to_string());
                }
            }
        }
        
        // Set summary
        if result.root_cause.is_some() {
            result.summary = format!(
                "Investigation complete. Root cause identified: {}",
                result.root_cause.as_ref().unwrap()
            );
        } else {
            result.summary = "Investigation complete. See findings and recommendations.".to_string();
        }
        
        Ok(())
    }
    
    /// Extract a section from the response text
    fn extract_section(&self, text: &str, markers: &[&str]) -> Option<String> {
        for marker in markers {
            if let Some(start_idx) = text.find(marker) {
                let start = start_idx + marker.len();
                let section_text = &text[start..];
                
                // Find the end of this section (next major marker or end of text)
                let end_markers = vec![
                    "\nROOT CAUSE:", "\nFINDINGS:", "\nRECOMMENDATIONS:", 
                    "\nAUTO-FIX:", "\nSUMMARY:", "\n\n\n"
                ];
                let mut end = section_text.len();
                
                for end_marker in end_markers {
                    if let Some(end_idx) = section_text.find(end_marker) {
                        end = end.min(end_idx);
                    }
                }
                
                return Some(section_text[..end].trim().to_string());
            }
        }
        
        None
    }
} 