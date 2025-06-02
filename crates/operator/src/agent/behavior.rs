//! Agent Behavior Abstraction
//! 
//! Core trait and types for pluggable agent behaviors

use std::sync::Arc;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use anyhow::Result;
use async_trait::async_trait;
use rig::completion::Message;
use chrono::{DateTime, Utc};

use super::{
    provider::{LLMProvider, LLMProviderType},
    safety::SafetyValidator,
    result::AgentResult,
};
use crate::agent::runtime::ToolType;
use kube::Client as K8sClient;

/// Shared context for all agent behaviors
#[derive(Clone)]
pub struct AgentContext {
    pub llm_provider: Arc<dyn LLMProvider>,
    pub llm_provider_type: Arc<LLMProviderType>,
    pub model: String,
    pub temperature: Option<f32>,
    pub tools: Arc<HashMap<String, ToolType>>,
    pub k8s_client: Option<K8sClient>,
    pub prometheus_endpoint: String,
    pub safety_validator: Arc<SafetyValidator>,
    // Additional resources like runbook access, config, etc.
}

/// Defines the types of input an agent behavior can process
#[derive(Debug, Clone)]
pub enum AgentInput {
    /// For interactive chat sessions
    ChatMessage {
        content: String,
        history: Vec<Message>,
        session_id: Option<String>,
        user_id: Option<String>,
    },
    /// For workflow-driven investigations
    InvestigationGoal {
        goal: String,
        initial_data: serde_json::Value,
        workflow_id: String,
        alert_context: Option<HashMap<String, String>>,
    },
    /// For resuming after human intervention
    ResumeInvestigation {
        original_goal: String,
        approval_response: HumanApprovalResponse,
        saved_state: serde_json::Value,
        workflow_id: String,
    },
}

/// Defines the types of output an agent behavior can produce
#[derive(Debug, Clone)]
pub enum AgentOutput {
    /// Response to a chat message
    ChatResponse {
        message: String,
        tool_calls_this_turn: Option<Vec<ToolCall>>,
        session_id: Option<String>,
        suggested_actions: Option<Vec<String>>,
    },
    /// Update during investigation
    InvestigationUpdate {
        status: String,
        findings_so_far: Vec<String>,
        workflow_id: String,
        progress_percentage: Option<u8>,
    },
    /// Request for human approval
    PendingHumanApproval {
        request_message: String,
        options: Vec<String>,
        current_investigation_state: serde_json::Value,
        workflow_id: String,
        risk_level: RiskLevel,
        timeout_seconds: Option<u64>,
    },
    /// Final investigation result
    FinalInvestigationResult(AgentResult),
    /// Error occurred
    Error {
        message: String,
        workflow_id: Option<String>,
        recoverable: bool,
    },
}

/// Human approval response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanApprovalResponse {
    pub approved: bool,
    pub feedback: Option<String>,
    pub selected_option: Option<String>,
    pub approver: String,
    pub approval_time: DateTime<Utc>,
}

/// Risk level for actions requiring approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Tool call made by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Core trait for all agent behaviors
#[async_trait]
pub trait AgentBehavior: Send + Sync {
    /// Handle an input and produce an output
    async fn handle(
        &self,
        input: AgentInput,
        context: Arc<AgentContext>,
    ) -> Result<AgentOutput>;
    
    /// Get the behavior type for identification
    fn behavior_type(&self) -> &'static str;
    
    /// Check if this behavior supports a given input type
    fn supports_input(&self, input: &AgentInput) -> bool;
}

/// Configuration for agent behaviors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBehaviorConfig {
    pub max_iterations: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub temperature: Option<f32>,
    pub system_prompt: Option<String>,
    pub require_approval_for: Vec<String>, // Tool names that require approval
}

impl Default for AgentBehaviorConfig {
    fn default() -> Self {
        Self {
            max_iterations: Some(10),
            timeout_seconds: Some(300),
            temperature: Some(0.7),
            system_prompt: None,
            require_approval_for: vec!["kubectl delete".to_string(), "kubectl patch".to_string()],
        }
    }
} 