//! Agent Runtime
//! 
//! Central runtime for agent creation and execution

use super::{
    behavior::{
        AgentBehavior, AgentContext, AgentInput, AgentOutput, 
        AgentBehaviorConfig, HumanApprovalResponse
    },
    chatbot::ChatbotAgent,
    investigator::InvestigatorAgent,
    provider::{self, LLMProvider, LLMConfig},
    result::{AgentResult, Finding, FindingSeverity, Recommendation, RiskLevel},
    safety::{SafetyValidator, SafetyConfig},
    tools::{
        kubectl::KubectlTool, promql::PromQLTool, curl::CurlTool, script::ScriptTool
    },
};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, error, debug};
use serde_json;
use rig::{
    completion::Prompt,
    providers::{anthropic, openai},
};
use regex::Regex;
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
    
    /// Set up kubectl tool with automatic configuration inference
    /// 
    /// This will try to:
    /// 1. Use the provided k8s_client if already set
    /// 2. Otherwise, use KubectlTool::infer() to automatically detect configuration
    pub async fn with_auto_kubectl(mut self) -> Self {
        // If we already have a k8s client, just return
        if self.k8s_client.is_some() {
            return self;
        }
        
        // Try to infer kubectl configuration
        match KubectlTool::infer().await {
            Ok(kubectl_tool) => {
                info!("Successfully inferred kubectl configuration");
                self.tools.insert("kubectl".to_string(), kubectl_tool.into());
                
                // Also try to create a k8s client for other uses
                if let Ok(client) = K8sClient::try_default().await {
                    self.k8s_client = Some(client);
                }
            }
            Err(e) => {
                warn!("Could not infer kubectl configuration: {}", e);
            }
        }
        
        self
    }
    
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Build the agent context from runtime configuration
    fn build_agent_context(&self) -> Arc<AgentContext> {
        // Create both the trait object and concrete type
        let llm_provider = match provider::create_provider(&self.llm_config) {
            Ok(provider) => provider,
            Err(e) => {
                error!("Failed to create LLM provider: {}", e);
                Arc::new(provider::MockProvider)
            }
        };
        
        let llm_provider_type = match provider::LLMProviderType::from_config(&self.llm_config) {
            Ok(provider_type) => Arc::new(provider_type),
            Err(e) => {
                error!("Failed to create LLM provider type: {}", e);
                Arc::new(provider::LLMProviderType::Mock)
            }
        };
        
        // If no tools were explicitly added but k8s client is available, add default tools
        let mut tools = self.tools.clone();
        if tools.is_empty() && self.k8s_client.is_some() {
            if let Some(k8s_client) = &self.k8s_client {
                tools.insert("kubectl".to_string(), KubectlTool::new(k8s_client.clone()).into());
                tools.insert("promql".to_string(), PromQLTool::new(self.prometheus_endpoint.clone()).into());
                tools.insert("curl".to_string(), CurlTool::new().into());
                tools.insert("script".to_string(), ScriptTool::new().into());
            }
        }
        
        Arc::new(AgentContext {
            llm_provider,
            llm_provider_type,
            model: self.llm_config.model.clone(),
            temperature: self.llm_config.temperature,
            tools: Arc::new(tools),
            k8s_client: self.k8s_client.clone(),
            prometheus_endpoint: self.prometheus_endpoint.clone(),
            safety_validator: Arc::new(self.safety_validator.clone()),
        })
    }
    
    /// Get a chatbot agent for interactive conversations
    pub fn get_chatbot_agent(&self) -> ChatbotAgent {
        ChatbotAgent::new(AgentBehaviorConfig::default())
    }
    
    /// Get a chatbot agent with custom configuration
    pub fn get_chatbot_agent_with_config(&self, config: AgentBehaviorConfig) -> ChatbotAgent {
        ChatbotAgent::new(config)
    }
    
    /// Get an investigator agent for autonomous investigations
    pub fn get_investigator_agent(&self) -> InvestigatorAgent {
        let mut config = AgentBehaviorConfig::default();
        config.max_iterations = Some(self.max_iterations);
        config.timeout_seconds = Some(self.timeout.as_secs());
        InvestigatorAgent::new(config)
    }
    
    /// Get an investigator agent with custom configuration
    pub fn get_investigator_agent_with_config(&self, config: AgentBehaviorConfig) -> InvestigatorAgent {
        InvestigatorAgent::new(config)
    }
    
    /// Execute an agent behavior with the given input
    pub async fn execute<A: AgentBehavior>(&self, agent: &A, input: AgentInput) -> Result<AgentOutput> {
        let context = self.build_agent_context();
        agent.handle(input, context).await
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
                agent.prompt(prompt)
                    .multi_turn(5)  // Allow tool usage
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
                agent.prompt(prompt)
                    .multi_turn(5)  // Allow tool usage
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
        info!("Starting agent investigation (using new InvestigatorAgent)");
        debug!("Goal: {}", goal);
        debug!("Context: {:?}", context);
        
        // Create investigator agent
        let investigator = self.get_investigator_agent();
        let agent_context = self.build_agent_context();
        
        // Create investigation input
        let input = AgentInput::InvestigationGoal {
            goal: goal.to_string(),
            initial_data: serde_json::to_value(&context)?,
            workflow_id: "backward-compat-investigation".to_string(),
            alert_context: Some(context),
        };
        
        // Run investigation
        let output = investigator.handle(input, agent_context).await?;
        
        // Handle the output
        match output {
            AgentOutput::FinalInvestigationResult(result) => Ok(result),
            AgentOutput::PendingHumanApproval { workflow_id, current_investigation_state, .. } => {
                // For backward compatibility, if approval is needed, auto-deny and return partial result
                info!("Investigation requires approval, auto-denying for backward compatibility");
                
                let denied_input = AgentInput::ResumeInvestigation {
                    original_goal: goal.to_string(),
                    approval_response: HumanApprovalResponse {
                        approved: false,
                        feedback: Some("Automatic denial for backward compatibility".to_string()),
                        selected_option: Some("Deny".to_string()),
                        approver: "system".to_string(),
                        approval_time: chrono::Utc::now(),
                    },
                    saved_state: current_investigation_state,
                    workflow_id,
                };
                
                let final_output = investigator.handle(denied_input, self.build_agent_context()).await?;
                match final_output {
                    AgentOutput::FinalInvestigationResult(result) => Ok(result),
                    _ => Err(anyhow::anyhow!("Unexpected output from investigator after denial")),
                }
            }
            AgentOutput::Error { message, .. } => {
                Err(anyhow::anyhow!("Investigation failed: {}", message))
            }
            _ => Err(anyhow::anyhow!("Unexpected output type from investigator")),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_agent_runtime_creation() {
        let config = LLMConfig {
            provider: "mock".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            endpoint: None,
            temperature: None,
            max_tokens: None,
            timeout_seconds: None,
        };
        
        let runtime = AgentRuntime::new(config).unwrap();
        assert!(runtime.k8s_client.is_none());
    }
    
    #[tokio::test]
    async fn test_chatbot_agent_creation() {
        let config = LLMConfig {
            provider: "mock".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            endpoint: None,
            temperature: None,
            max_tokens: None,
            timeout_seconds: None,
        };
        
        let runtime = AgentRuntime::new(config).unwrap();
        let chatbot = runtime.get_chatbot_agent();
        
        // Test that we can execute a chat message
        let input = AgentInput::ChatMessage {
            content: "Hello, can you help me check pod status?".to_string(),
            history: vec![],
            session_id: Some("test-session".to_string()),
            user_id: Some("test-user".to_string()),
        };
        
        let output = runtime.execute(&chatbot, input).await.unwrap();
        
        match output {
            AgentOutput::ChatResponse { message, .. } => {
                assert!(message.contains("mock mode"));
            }
            _ => panic!("Expected ChatResponse"),
        }
    }
    
    #[tokio::test]
    async fn test_investigator_agent_creation() {
        let config = LLMConfig {
            provider: "mock".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            endpoint: None,
            temperature: None,
            max_tokens: None,
            timeout_seconds: None,
        };
        
        let runtime = AgentRuntime::new(config).unwrap();
        let investigator = runtime.get_investigator_agent();
        
        // Test investigation
        let input = AgentInput::InvestigationGoal {
            goal: "Investigate PodCrashLooping alert".to_string(),
            initial_data: serde_json::json!({"alert": "PodCrashLooping"}),
            workflow_id: "test-workflow".to_string(),
            alert_context: None,
        };
        
        let output = runtime.execute(&investigator, input).await.unwrap();
        
        match output {
            AgentOutput::FinalInvestigationResult(result) => {
                assert!(result.root_cause.is_some());
                assert!(!result.findings.is_empty());
            }
            _ => panic!("Expected FinalInvestigationResult"),
        }
    }
    
    #[tokio::test]
    async fn test_backward_compatibility() {
        let config = LLMConfig {
            provider: "mock".to_string(),
            model: "test-model".to_string(),
            api_key: None,
            endpoint: None,
            temperature: None,
            max_tokens: None,
            timeout_seconds: None,
        };
        
        let runtime = AgentRuntime::new(config).unwrap();
        
        // Test the backward-compatible investigate method
        let mut context = HashMap::new();
        context.insert("alert_name".to_string(), "PodCrashLooping".to_string());
        
        let result = runtime.investigate("Check why pod is crashing", context).await.unwrap();
        
        assert!(result.root_cause.is_some());
        assert!(result.can_auto_fix);
    }
} 