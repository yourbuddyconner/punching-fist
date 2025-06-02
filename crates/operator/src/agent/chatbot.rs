//! Chatbot Agent Implementation
//! 
//! Synchronous, interactive agent for chat-based interfaces

use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, debug, warn};
use rig::completion::Prompt;

use super::{
    behavior::{AgentBehavior, AgentInput, AgentOutput, AgentContext, ToolCall, AgentBehaviorConfig},
    provider::{LLMProviderType, map_anthropic_model},
};
use crate::agent::runtime::ToolType;

/// Chatbot agent for interactive conversations
pub struct ChatbotAgent {
    config: AgentBehaviorConfig,
}

impl ChatbotAgent {
    /// Create a new chatbot agent
    pub fn new(config: AgentBehaviorConfig) -> Self {
        Self { config }
    }
    
    /// Build system prompt for the chatbot
    fn build_system_prompt(&self) -> String {
        let base_prompt = self.config.system_prompt.clone().unwrap_or_else(|| {
            "You are a helpful Kubernetes operations assistant. \
            You can answer questions about the cluster state, help debug issues, \
            and suggest solutions. You have access to various tools to inspect \
            the cluster, query metrics, and check logs. Always be concise and helpful.
            When tools are run do your best to describe the output in a table if necessary."
                .to_string()
        });
        
        // Add cluster context if available
        // NOTE: In a real implementation, you would fetch this at agent initialization
        // and store it in the ChatbotAgent struct or AgentContext
        let cluster_context = r#"
## Cluster Context
- Current cluster: (will be populated at runtime)
- Available namespaces: (will be populated at runtime)
- Supported kubectl resources: pods, namespaces, services, deployments, all
- Special notes: Use 'get all' to see all workload resources in a namespace"#;
        
        format!("{}\n\n{}", base_prompt, cluster_context)
    }
    
    /// Process a chat message using Rig's Chat trait
    async fn process_chat_message(
        &self,
        content: &str,
        history: Vec<rig::completion::Message>,
        context: Arc<AgentContext>,
    ) -> Result<(String, Option<Vec<ToolCall>>)> {
        info!("Processing chat message: {}", content);
        
        // Build the chat with tools based on the provider
        match &*context.llm_provider_type {
            LLMProviderType::Anthropic(client) => {
                // Map the model name to correct Anthropic API identifier
                let anthropic_model = map_anthropic_model(&context.model);
                
                let mut builder = client
                    .agent(anthropic_model)
                    .preamble(&self.build_system_prompt());
                
                // Add tools from context
                for (name, tool) in context.tools.iter() {
                    debug!("Adding tool to chatbot: {}", name);
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
                
                let agent = builder.build();
                
                // Use Rig's prompt method with history and multi-turn enabled
                let mut history_clone = history.clone();
                let response = agent.prompt(content)
                    .with_history(&mut history_clone)
                    .multi_turn(10)  // Allow at least 1 turn for tool calls
                    .await
                    .map_err(|e| anyhow::anyhow!("Chat failed: {:?}", e))?;
                
                // TODO: Extract tool calls from the response if Rig provides them
                Ok((response, None))
            }
            LLMProviderType::OpenAI(client) => {
                // For OpenAI, use the model name directly (no mapping needed)
                let mut builder = client
                    .agent(&context.model)
                    .preamble(&self.build_system_prompt());
                
                // Add tools from context
                for (name, tool) in context.tools.iter() {
                    debug!("Adding tool to chatbot: {}", name);
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
                
                let agent = builder.build();
                
                // Use Rig's prompt method with history and multi-turn enabled
                let mut history_clone = history.clone();
                let response = agent.prompt(content)
                    .with_history(&mut history_clone)
                    .multi_turn(10)  // Allow at least 1 turn for tool calls
                    .await
                    .map_err(|e| anyhow::anyhow!("Chat failed: {:?}", e))?;
                
                Ok((response, None))
            }
            LLMProviderType::Mock => {
                // For mock or unsupported providers, return a simple response
                Ok((
                    format!("I received your message: '{}'. However, I'm currently running in mock mode.", content),
                    None
                ))
            }
        }
    }
    
    /// Generate suggested actions based on the conversation
    fn generate_suggestions(&self, response: &str) -> Option<Vec<String>> {
        // Simple heuristic-based suggestions
        let mut suggestions = Vec::new();
        
        if response.contains("pod") && response.contains("crash") {
            suggestions.push("Check pod logs: kubectl logs <pod-name>".to_string());
            suggestions.push("Describe pod: kubectl describe pod <pod-name>".to_string());
        }
        
        if response.contains("memory") || response.contains("OOM") {
            suggestions.push("Check memory usage: kubectl top pods".to_string());
            suggestions.push("Increase memory limits in deployment".to_string());
        }
        
        if response.contains("CPU") || response.contains("throttling") {
            suggestions.push("Check CPU usage: kubectl top pods".to_string());
            suggestions.push("Consider horizontal pod autoscaling".to_string());
        }
        
        if suggestions.is_empty() {
            None
        } else {
            Some(suggestions)
        }
    }
}

#[async_trait]
impl AgentBehavior for ChatbotAgent {
    async fn handle(
        &self,
        input: AgentInput,
        context: Arc<AgentContext>,
    ) -> Result<AgentOutput> {
        match input {
            AgentInput::ChatMessage {
                content,
                history,
                session_id,
                user_id,
            } => {
                debug!("Handling chat message from user: {:?}", user_id);
                
                // Process the message
                let (response, tool_calls) = self.process_chat_message(&content, history, context).await?;
                
                // Generate suggestions based on the response
                let suggested_actions = self.generate_suggestions(&response);
                
                Ok(AgentOutput::ChatResponse {
                    message: response,
                    tool_calls_this_turn: tool_calls,
                    session_id,
                    suggested_actions,
                })
            }
            _ => {
                warn!("ChatbotAgent received unsupported input type");
                Ok(AgentOutput::Error {
                    message: "ChatbotAgent only supports ChatMessage inputs".to_string(),
                    workflow_id: None,
                    recoverable: false,
                })
            }
        }
    }
    
    fn behavior_type(&self) -> &'static str {
        "chatbot"
    }
    
    fn supports_input(&self, input: &AgentInput) -> bool {
        matches!(input, AgentInput::ChatMessage { .. })
    }
} 