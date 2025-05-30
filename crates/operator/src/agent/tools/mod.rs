//! Agent Tools Module
//! 
//! Provides tools that LLM agents can use to investigate alerts and perform actions.

pub mod kubectl;
pub mod promql;
pub mod curl;
pub mod script;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
// Commenting out Rig tool integration for now - will implement in provider
// use rig::tool::Tool as RigTool;

/// Result from tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Common trait for all agent tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name
    fn name(&self) -> &str;
    
    /// Get the tool description for the LLM
    fn description(&self) -> &str;
    
    /// Execute the tool with the given command/query
    async fn execute(&self, input: &str) -> Result<ToolResult>;
    
    /// Validate if the command is safe to execute
    fn validate(&self, input: &str) -> Result<()>;
}

/* Commented out until we properly integrate with Rig's tool system
/// Convert our Tool trait to Rig's Tool trait
pub struct RigToolAdapter<T: Tool> {
    tool: T,
}

impl<T: Tool> RigToolAdapter<T> {
    pub fn new(tool: T) -> Self {
        Self { tool }
    }
}

#[async_trait]
impl<T: Tool> RigTool for RigToolAdapter<T> {
    async fn call(&self, input: serde_json::Value) -> Result<serde_json::Value> {
        let input_str = input.as_str()
            .ok_or_else(|| anyhow::anyhow!("Tool input must be a string"))?;
        
        // Validate input first
        self.tool.validate(input_str)?;
        
        // Execute the tool
        let result = self.tool.execute(input_str).await?;
        
        // Convert to JSON
        Ok(serde_json::to_value(result)?)
    }
    
    fn description(&self) -> &str {
        self.tool.description()
    }
}
*/ 