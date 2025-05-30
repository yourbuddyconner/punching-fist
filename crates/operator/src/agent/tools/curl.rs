//! Curl Tool for HTTP Requests
//! 
//! Allows agents to make HTTP requests for health checks and API calls.

use super::{Tool, ToolResult};
use anyhow::Result;
use async_trait::async_trait;

/// Curl tool for HTTP requests
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
}

#[async_trait]
impl Tool for CurlTool {
    fn name(&self) -> &str {
        "curl"
    }
    
    fn description(&self) -> &str {
        "Make HTTP requests for health checks and API calls. \
         Example: 'curl http://service:8080/health'"
    }
    
    async fn execute(&self, input: &str) -> Result<ToolResult> {
        // TODO: Implement actual HTTP request logic
        Ok(ToolResult {
            success: true,
            output: format!("Curl tool called with: {}", input),
            error: None,
            metadata: None,
        })
    }
    
    fn validate(&self, input: &str) -> Result<()> {
        // TODO: Validate URL against allowed domains
        Ok(())
    }
} 