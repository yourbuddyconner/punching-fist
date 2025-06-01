//! Curl Tool for HTTP Requests
//! 
//! Allows agents to make HTTP requests for health checks and API calls.

use super::{ToolResult, ToolArgs, ToolError};
use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool as RigTool;

/// Curl tool for HTTP requests
#[derive(Clone)]
pub struct CurlTool {
    allowed_domains: Vec<String>,
}

impl CurlTool {
    pub fn new() -> Self {
        Self {
            allowed_domains: vec!["localhost".to_string()],
        }
    }
    
    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = domains;
        self
    }
    
    fn validate(&self, input: &str) -> Result<()> {
        // TODO: Validate URL against allowed domains
        Ok(())
    }
}

impl RigTool for CurlTool {
    const NAME: &'static str = "curl";
    
    type Error = ToolError;
    type Args = ToolArgs;
    type Output = ToolResult;
    
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Make HTTP requests for health checks and API calls. \
                         Example: 'curl http://service:8080/health'".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The URL to request (e.g., 'http://service:8080/health')"
                    }
                },
                "required": ["command"]
            }),
        }
    }
    
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.validate(&args.command)
            .map_err(|e| ToolError::ValidationError(e.to_string()))?;
        
        // TODO: Implement actual HTTP request logic
        Ok(ToolResult {
            success: true,
            output: format!("Curl tool called with: {}", args.command),
            error: None,
            metadata: None,
        })
    }
} 