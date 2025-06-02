//! Curl Tool for HTTP Requests
//! 
//! Allows agents to make HTTP requests for health checks and API calls.

use super::{ToolResult, ToolArgs, ToolError};
use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool as RigTool;
use reqwest;
use url::Url;
use std::time::Duration;

/// Curl tool for HTTP requests
#[derive(Clone)]
pub struct CurlTool {
    allowed_domains: Vec<String>,
}

impl CurlTool {
    pub fn new() -> Self {
        Self {
            // Allow common domains by default, including httpbin for testing
            allowed_domains: vec![
                "localhost".to_string(),
                "127.0.0.1".to_string(),
                "httpbin.org".to_string(),
                "connerswann.me".to_string(),
            ],
        }
    }
    
    pub fn with_allowed_domains(mut self, domains: Vec<String>) -> Self {
        self.allowed_domains = domains;
        self
    }
    
    fn validate(&self, input: &str) -> Result<()> {
        // Parse URL
        let url = Url::parse(input)
            .map_err(|e| anyhow::anyhow!("Invalid URL: {}", e))?;
        
        // Check if host is allowed
        if let Some(host) = url.host_str() {
            let is_allowed = self.allowed_domains.iter().any(|domain| {
                host == domain || host.ends_with(&format!(".{}", domain))
            });
            
            if !is_allowed {
                return Err(anyhow::anyhow!(
                    "Domain '{}' is not in the allowed list: {:?}",
                    host,
                    self.allowed_domains
                ));
            }
        } else {
            return Err(anyhow::anyhow!("URL has no host"));
        }
        
        // Only allow HTTP and HTTPS
        if !["http", "https"].contains(&url.scheme()) {
            return Err(anyhow::anyhow!("Only HTTP and HTTPS protocols are allowed"));
        }
        
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
        
        // Create HTTP client with timeout
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| ToolError::ExecutionError(format!("Failed to create HTTP client: {}", e)))?;
        
        // Make the request
        match client.get(&args.command).send().await {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();
                
                // Try to get response body
                let body = match response.text().await {
                    Ok(text) => {
                        // Truncate very long responses
                        if text.len() > 1000 {
                            format!("{}... (truncated, {} total bytes)", &text[..1000], text.len())
                        } else {
                            text
                        }
                    }
                    Err(e) => format!("<Error reading response body: {}>", e),
                };
                
                // Format output similar to curl
                let mut output = format!("HTTP/{} {}\n", 
                    if status.as_u16() < 200 { "1.1" } else { "2.0" },
                    status
                );
                
                // Add some key headers
                if let Some(content_type) = headers.get("content-type") {
                    output.push_str(&format!("Content-Type: {}\n", content_type.to_str().unwrap_or("<invalid>")));
                }
                if let Some(content_length) = headers.get("content-length") {
                    output.push_str(&format!("Content-Length: {}\n", content_length.to_str().unwrap_or("<invalid>")));
                }
                
                output.push_str("\n");
                output.push_str(&body);
                
                Ok(ToolResult {
                    success: status.is_success(),
                    output,
                    error: if !status.is_success() {
                        Some(format!("HTTP error: {}", status))
                    } else {
                        None
                    },
                    metadata: Some(serde_json::json!({
                        "status_code": status.as_u16(),
                        "url": args.command,
                    })),
                })
            }
            Err(e) => {
                let error_msg = if e.is_timeout() {
                    "Request timed out after 10 seconds".to_string()
                } else if e.is_connect() {
                    format!("Failed to connect: {}", e)
                } else {
                    format!("Request failed: {}", e)
                };
                
                Ok(ToolResult {
                    success: false,
                    output: error_msg.clone(),
                    error: Some(error_msg),
                    metadata: Some(serde_json::json!({
                        "url": args.command,
                        "error_type": if e.is_timeout() { "timeout" } 
                                     else if e.is_connect() { "connection" }
                                     else { "other" },
                    })),
                })
            }
        }
    }
} 