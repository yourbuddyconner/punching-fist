//! Investigator Agent Implementation
//! 
//! Autonomous agent for workflow-driven investigations with human-in-the-loop support

use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, debug, warn, error};
use serde_json;
use regex::Regex;
use chrono::Utc;
use rig::{completion::Prompt, providers::{anthropic, openai}};

use super::{
    behavior::{
        AgentBehavior, AgentInput, AgentOutput, AgentContext, ToolCall, 
        AgentBehaviorConfig, RiskLevel, HumanApprovalResponse
    },
    provider::{LLMProvider, LLMProviderType, map_anthropic_model},
    result::{AgentResult, Finding, FindingSeverity, Recommendation, RiskLevel as ResultRiskLevel, ActionTaken},
    templates,
    safety::SafetyValidator,
};
use crate::agent::runtime::ToolType;

/// Investigator agent for autonomous investigations
pub struct InvestigatorAgent {
    config: AgentBehaviorConfig,
}

impl InvestigatorAgent {
    /// Create a new investigator agent
    pub fn new(config: AgentBehaviorConfig) -> Self {
        Self { config }
    }
    
    /// Build system prompt for investigation
    fn build_investigation_prompt(&self, goal: &str, context: &serde_json::Value) -> String {
        let system_prompt = self.config.system_prompt.clone().unwrap_or_else(|| {
            templates::INVESTIGATION_SYSTEM_PROMPT.to_string()
        });
        
        format!(
            "{}\n\n\
            Investigation Goal: {}\n\n\
            Context:\n{}\n\n\
            Please investigate this issue step by step. Use the available tools to gather evidence. \
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
            system_prompt,
            goal,
            serde_json::to_string_pretty(context).unwrap_or_default()
        )
    }
    
    /// Check if an action requires approval
    fn requires_approval(&self, action: &str) -> bool {
        self.config.require_approval_for.iter().any(|pattern| {
            action.contains(pattern)
        })
    }
    
    /// Determine risk level for an action
    fn assess_risk_level(&self, action: &str) -> RiskLevel {
        if action.contains("delete") || action.contains("remove") {
            RiskLevel::High
        } else if action.contains("patch") || action.contains("scale") {
            RiskLevel::Medium
        } else if action.contains("describe") || action.contains("get") || action.contains("logs") {
            RiskLevel::Low
        } else {
            RiskLevel::Medium
        }
    }
    
    /// Run investigation using Rig's agent
    async fn run_investigation(
        &self,
        goal: &str,
        context: &serde_json::Value,
        agent_context: Arc<AgentContext>,
    ) -> Result<String> {
        let prompt = self.build_investigation_prompt(goal, context);
        
        // Create initial investigation message
        let investigation_message = format!(
            "Please start investigating this issue. Goal: {}\n\nBegin by analyzing the available context and using the appropriate tools to gather evidence.",
            goal
        );
        
        match &*agent_context.llm_provider_type {
            LLMProviderType::Anthropic(client) => {
                // Map the model name to correct Anthropic API identifier
                let anthropic_model = map_anthropic_model(&agent_context.model);
                
                let mut builder = client
                    .agent(anthropic_model)
                    .preamble(&prompt);
                
                // Add tools
                for (name, tool) in agent_context.tools.iter() {
                    debug!("Adding tool to investigator: {}", name);
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
                }
                
                let agent = builder
                    .build();
                
                // Try investigation with error recovery
                match agent.prompt(&investigation_message)
                    .multi_turn(10)
                    .await
                {
                    Ok(response) => Ok(response),
                    Err(e) => {
                        // Check if this is a tool validation error that we can recover from
                        let error_msg = format!("{:?}", e);
                        if error_msg.contains("ToolCallError") && (
                            error_msg.contains("not allowed") || 
                            error_msg.contains("ValidationError") ||
                            error_msg.contains("Allowed verbs are")
                        ) {
                            warn!("Tool validation error encountered, attempting recovery: {}", error_msg);
                            
                            // Create a recovery prompt that informs the model about the tool constraints
                            let recovery_prompt = format!(
                                "{}\n\nIMPORTANT: Some tools have constraints. For kubectl, only these verbs are allowed: get, describe, logs, events, top. \
                                Do NOT attempt to use delete, patch, or other modification commands.\n\n\
                                Please complete your investigation using only the available tools and provide your analysis.",
                                prompt
                            );
                            
                            // Try again with the constraint-aware prompt
                            let mut recovery_builder = client
                                .agent(anthropic_model)
                                .preamble(&recovery_prompt);
                                
                            // Add all tools to recovery agent
                            for (name, tool) in agent_context.tools.iter() {
                                debug!("Adding tool to recovery investigator: {}", name);
                                match tool {
                                    ToolType::Kubectl(kubectl_tool) => {
                                        recovery_builder = recovery_builder.tool(kubectl_tool.clone());
                                    }
                                    ToolType::PromQL(promql_tool) => {
                                        recovery_builder = recovery_builder.tool(promql_tool.clone());
                                    }
                                    ToolType::Curl(curl_tool) => {
                                        recovery_builder = recovery_builder.tool(curl_tool.clone());
                                    }
                                    ToolType::Script(script_tool) => {
                                        recovery_builder = recovery_builder.tool(script_tool.clone());
                                    }
                                }
                            }
                            
                            let recovery_agent = recovery_builder.build();
                            
                            match recovery_agent.prompt(&investigation_message)
                                .multi_turn(5)  // Fewer turns for recovery attempt
                                .await
                            {
                                Ok(response) => {
                                    info!("Investigation recovered successfully after tool validation error");
                                    Ok(response)
                                }
                                Err(recovery_err) => {
                                    error!("Investigation failed even after recovery attempt: {:?}", recovery_err);
                                    // Return a partial result based on what we know
                                    Ok(format!(
                                        "Investigation encountered tool constraints but provided partial analysis:\n\n\
                                        ROOT CAUSE: Unable to complete full investigation due to tool limitations\n\n\
                                        FINDINGS:\n\
                                        - Investigation was limited by available tool permissions\n\
                                        - Only read-only operations are available (get, describe, logs, events, top)\n\
                                        - Original error: {}\n\n\
                                        RECOMMENDATIONS:\n\
                                        - Manual investigation required for actions requiring elevated permissions\n\
                                        - Review kubectl tool configuration to allow necessary operations\n\
                                        - Use available tools to gather more diagnostic information\n\n\
                                        AUTO-FIX: no",
                                        error_msg
                                    ))
                                }
                            }
                        } else {
                            Err(anyhow::anyhow!("Investigation failed: {:?}", e))
                        }
                    }
                }
            }
            LLMProviderType::OpenAI(client) => {
                // For OpenAI, use the model name directly (no mapping needed)
                let mut builder = client
                    .agent(&agent_context.model)
                    .preamble(&prompt);
                
                // Add tools
                for (name, tool) in agent_context.tools.iter() {
                    debug!("Adding tool to investigator: {}", name);
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
                }
                
                let agent = builder
                    .build();
                
                // Try investigation with error recovery (similar logic for OpenAI)
                match agent.prompt(&investigation_message)
                    .multi_turn(10)
                    .await
                {
                    Ok(response) => Ok(response),
                    Err(e) => {
                        let error_msg = format!("{:?}", e);
                        if error_msg.contains("ToolCallError") && (
                            error_msg.contains("not allowed") || 
                            error_msg.contains("ValidationError") ||
                            error_msg.contains("Allowed verbs are")
                        ) {
                            warn!("Tool validation error encountered, attempting recovery: {}", error_msg);
                            
                            let recovery_prompt = format!(
                                "{}\n\nIMPORTANT: Some tools have constraints. For kubectl, only these verbs are allowed: get, describe, logs, events, top. \
                                Do NOT attempt to use delete, patch, or other modification commands.\n\n\
                                Please complete your investigation using only the available tools and provide your analysis.",
                                prompt
                            );
                            
                            let mut recovery_builder = client
                                .agent(&agent_context.model)
                                .preamble(&recovery_prompt);
                                
                            // Add all tools to recovery agent
                            for (name, tool) in agent_context.tools.iter() {
                                debug!("Adding tool to recovery investigator: {}", name);
                                match tool {
                                    ToolType::Kubectl(kubectl_tool) => {
                                        recovery_builder = recovery_builder.tool(kubectl_tool.clone());
                                    }
                                    ToolType::PromQL(promql_tool) => {
                                        recovery_builder = recovery_builder.tool(promql_tool.clone());
                                    }
                                    ToolType::Curl(curl_tool) => {
                                        recovery_builder = recovery_builder.tool(curl_tool.clone());
                                    }
                                    ToolType::Script(script_tool) => {
                                        recovery_builder = recovery_builder.tool(script_tool.clone());
                                    }
                                }
                            }
                            
                            let recovery_agent = recovery_builder.build();
                            
                            match recovery_agent.prompt(&investigation_message)
                                .multi_turn(5)
                                .await
                            {
                                Ok(response) => {
                                    info!("Investigation recovered successfully after tool validation error");
                                    Ok(response)
                                }
                                Err(_) => {
                                    Ok(format!(
                                        "Investigation encountered tool constraints but provided partial analysis:\n\n\
                                        ROOT CAUSE: Unable to complete full investigation due to tool limitations\n\n\
                                        FINDINGS:\n\
                                        - Investigation was limited by available tool permissions\n\
                                        - Only read-only operations are available (get, describe, logs, events, top)\n\
                                        - Original error: {}\n\n\
                                        RECOMMENDATIONS:\n\
                                        - Manual investigation required for actions requiring elevated permissions\n\
                                        - Review kubectl tool configuration to allow necessary operations\n\
                                        - Use available tools to gather more diagnostic information\n\n\
                                        AUTO-FIX: no",
                                        error_msg
                                    ))
                                }
                            }
                        } else {
                            Err(anyhow::anyhow!("Investigation failed: {:?}", e))
                        }
                    }
                }
            }
            LLMProviderType::Mock => {
                // Mock response for testing
                Ok(self.mock_investigation_response(goal))
            }
        }
    }
    
    /// Mock investigation response for testing
    fn mock_investigation_response(&self, goal: &str) -> String {
        // Check for PodCrashLooping in the goal or initial data
        if goal.to_lowercase().contains("podcrashlooping") || goal.to_lowercase().contains("pod") && goal.to_lowercase().contains("crash") {
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
        } else if goal.to_lowercase().contains("highcpuusage") || goal.to_lowercase().contains("cpu") {
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
    
    /// Parse investigation response into structured result
    fn parse_investigation_response(&self, response: &str) -> AgentResult {
        let mut result = AgentResult::new("Investigation complete".to_string());
        
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
                        risk_level: ResultRiskLevel::Low,
                        requires_approval: self.requires_approval(rec_text),
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
                let kubectl_regex = Regex::new(r"kubectl\s+[^\n]+").unwrap();
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
        }
        
        result
    }
    
    /// Extract a section from the response text
    fn extract_section(&self, text: &str, markers: &[&str]) -> Option<String> {
        for marker in markers {
            if let Some(start_idx) = text.find(marker) {
                let start = start_idx + marker.len();
                let section_text = &text[start..];
                
                // Find the end of this section
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

#[async_trait]
impl AgentBehavior for InvestigatorAgent {
    async fn handle(
        &self,
        input: AgentInput,
        context: Arc<AgentContext>,
    ) -> Result<AgentOutput> {
        match input {
            AgentInput::InvestigationGoal {
                goal,
                initial_data,
                workflow_id,
                alert_context,
            } => {
                info!("Starting investigation for workflow {}: {}", workflow_id, goal);
                
                // Merge alert context into initial data if provided
                let mut investigation_context = initial_data.clone();
                if let Some(alert_ctx) = alert_context {
                    if let serde_json::Value::Object(ref mut map) = investigation_context {
                        map.insert("alert_context".to_string(), serde_json::to_value(alert_ctx)?);
                    }
                }
                
                // Run the investigation
                let response = self.run_investigation(&goal, &investigation_context, context.clone()).await?;
                debug!("Investigation response: {}", response);
                
                // Check if the response contains actions that require approval
                if response.contains("kubectl delete") || response.contains("kubectl patch") {
                    if self.requires_approval(&response) {
                        // Extract the proposed action
                        let kubectl_regex = Regex::new(r"kubectl\s+[^\n]+").unwrap();
                        let proposed_action = kubectl_regex
                            .find(&response)
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_else(|| "Unknown action".to_string());
                        
                        let risk_level = self.assess_risk_level(&proposed_action);
                        
                        return Ok(AgentOutput::PendingHumanApproval {
                            request_message: format!(
                                "Investigation found a potential fix that requires approval:\n\n{}\n\nProposed action: {}",
                                response, proposed_action
                            ),
                            options: vec!["Approve".to_string(), "Deny".to_string(), "Modify".to_string()],
                            current_investigation_state: serde_json::json!({
                                "response": response,
                                "goal": goal,
                                "proposed_action": proposed_action,
                            }),
                            workflow_id,
                            risk_level,
                            timeout_seconds: Some(300), // 5 minute timeout
                        });
                    }
                }
                
                // Parse and return the final result
                let result = self.parse_investigation_response(&response);
                Ok(AgentOutput::FinalInvestigationResult(result))
            }
            AgentInput::ResumeInvestigation {
                original_goal,
                approval_response,
                saved_state,
                workflow_id,
            } => {
                info!("Resuming investigation for workflow {}", workflow_id);
                
                let response = saved_state.get("response")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let proposed_action = saved_state.get("proposed_action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                let mut result = self.parse_investigation_response(response);
                
                if approval_response.approved {
                    result.add_action(ActionTaken {
                        tool: "human_approval".to_string(),
                        command: proposed_action.to_string(),
                        timestamp: Utc::now(),
                        success: true,
                        output_summary: format!("Approved by {}", approval_response.approver),
                    });
                    
                    if let Some(feedback) = &approval_response.feedback {
                        result.summary = format!("{}\nHuman feedback: {}", result.summary, feedback);
                    }
                } else {
                    result.can_auto_fix = false;
                    result.fix_command = None;
                    result.summary = format!(
                        "{}\nHuman denied the proposed fix. Manual intervention required.",
                        result.summary
                    );
                }
                
                Ok(AgentOutput::FinalInvestigationResult(result))
            }
            _ => {
                warn!("InvestigatorAgent received unsupported input type");
                Ok(AgentOutput::Error {
                    message: "InvestigatorAgent only supports InvestigationGoal and ResumeInvestigation inputs".to_string(),
                    workflow_id: None,
                    recoverable: false,
                })
            }
        }
    }
    
    fn behavior_type(&self) -> &'static str {
        "investigator"
    }
    
    fn supports_input(&self, input: &AgentInput) -> bool {
        matches!(
            input,
            AgentInput::InvestigationGoal { .. } | AgentInput::ResumeInvestigation { .. }
        )
    }
} 