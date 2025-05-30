//! LLM Provider Abstraction
//! 
//! Provides a unified interface for different LLM providers using Rig.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Import from rig
use rig::completion::Prompt;
use rig::providers::{anthropic, openai};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    pub provider: String,
    pub endpoint: Option<String>,
    pub model: String,
    pub api_key: Option<String>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub timeout_seconds: Option<u64>,
}

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: "anthropic".to_string(),
            endpoint: None,
            model: "claude-3-5-sonnet".to_string(),
            api_key: None,
            temperature: Some(0.7),
            max_tokens: Some(4096),
            timeout_seconds: Some(300),
        }
    }
}

/// Trait for LLM providers that can handle prompts
#[async_trait::async_trait]
pub trait LLMProvider: Send + Sync {
    /// Send a prompt to the LLM and get a response
    async fn prompt(&self, prompt: &str) -> Result<String>;
}

/// Anthropic Claude provider using Rig
pub struct AnthropicProvider {
    client: anthropic::Client,
    model: String,
}

impl AnthropicProvider {
    pub fn new(api_key: Option<String>, model: &str) -> Result<Self> {
        let client = if let Some(key) = api_key {
            // Use the documented API with all 4 parameters
            anthropic::Client::new(
                &key,
                "https://api.anthropic.com",  // Default Anthropic API base URL
                None,  // No betas
                anthropic::ANTHROPIC_VERSION_LATEST,  // Latest version
            )
        } else {
            // Use from_env() which will read ANTHROPIC_API_KEY
            anthropic::Client::from_env()
        };
        
        Ok(Self { 
            client,
            model: model.to_string(),
        })
    }
    
    /// Map model name to Rig's model constant
    fn get_model_id(&self) -> &'static str {
        match self.model.as_str() {
            "claude-3-5-sonnet" | "claude-3-5-sonnet-20241022" => anthropic::CLAUDE_3_5_SONNET,
            "claude-3-7-sonnet" => anthropic::CLAUDE_3_7_SONNET,
            "claude-3-haiku" | "claude-3-haiku-20240307" => anthropic::CLAUDE_3_HAIKU,
            "claude-3-opus" | "claude-3-opus-20240229" => anthropic::CLAUDE_3_OPUS,
            "claude-3-sonnet" | "claude-3-sonnet-20240229" => anthropic::CLAUDE_3_SONNET,
            _ => anthropic::CLAUDE_3_5_SONNET, // Default
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for AnthropicProvider {
    async fn prompt(&self, prompt: &str) -> Result<String> {
        // Create a completion model
        let model = self.client.completion_model(self.get_model_id());
        
        // Create an agent from the model (following the OpenAI pattern)
        let agent = self.client
            .agent(self.get_model_id())
            .build();
        
        // Send the prompt
        let response = agent
            .prompt(prompt)
            .await
            .map_err(|e| anyhow::anyhow!("Anthropic API error: {:?}", e))?;
        
        Ok(response)
    }
}

/// OpenAI provider using Rig
pub struct OpenAIProvider {
    client: openai::Client,
    model: String,
}

impl OpenAIProvider {
    pub fn new(api_key: Option<String>, model: &str) -> Result<Self> {
        let client = if let Some(key) = api_key {
            openai::Client::new(&key)
        } else {
            // This will use OPENAI_API_KEY env var
            openai::Client::from_env()
        };
        
        Ok(Self { 
            client,
            model: model.to_string(),
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
    async fn prompt(&self, prompt: &str) -> Result<String> {
        // Create an agent for this specific prompt
        let agent = self.client
            .agent(&self.model)
            .build();
            
        let response = agent
            .prompt(prompt)
            .await
            .map_err(|e| anyhow::anyhow!("OpenAI API error: {:?}", e))?;
        
        Ok(response)
    }
}

/// Mock provider for testing
pub struct MockProvider;

#[async_trait::async_trait]
impl LLMProvider for MockProvider {
    async fn prompt(&self, prompt: &str) -> Result<String> {
        // Return a mock response based on the prompt
        if prompt.contains("PodCrashLooping") {
            Ok("Based on my investigation:\n\n\
                Tool: kubectl describe pod\n\
                Result: The pod is experiencing an OutOfMemoryError with exit code 137 (OOMKilled).\n\n\
                Tool: kubectl logs\n\
                Result: Java heap space OutOfMemoryError detected in the application logs.\n\n\
                Tool: promql query\n\
                Result: Memory usage shows the container consistently hitting its 512MB limit.\n\n\
                Root Cause: The container memory limit of 512MB is insufficient for the Java application's heap requirements.\n\n\
                Recommendation: Increase the memory limit to 1GB.".to_string())
        } else if prompt.contains("HighCPUUsage") {
            Ok("Based on my investigation:\n\n\
                Tool: promql query\n\
                Result: CPU usage at 95% sustained over the past 5 minutes.\n\n\
                Tool: kubectl top\n\
                Result: All pods are consuming high CPU, indicating legitimate load.\n\n\
                Root Cause: The service is experiencing high legitimate traffic load.\n\n\
                Recommendation: Scale the deployment to handle the increased load.".to_string())
        } else {
            Ok(format!("Investigating: {}...\n\nUnable to determine root cause. Manual investigation required.", 
                &prompt.chars().take(50).collect::<String>()))
        }
    }
}

/// Create a provider from configuration
pub fn create_provider(config: &LLMConfig) -> Result<Arc<dyn LLMProvider>> {
    match config.provider.as_str() {
        "anthropic" | "claude" => {
            let provider = AnthropicProvider::new(config.api_key.clone(), &config.model)?;
            Ok(Arc::new(provider))
        }
        "openai" => {
            let provider = OpenAIProvider::new(config.api_key.clone(), &config.model)?;
            Ok(Arc::new(provider))
        }
        "mock" => Ok(Arc::new(MockProvider)),
        _ => {
            // Default to mock for now
            Ok(Arc::new(MockProvider))
        }
    }
} 