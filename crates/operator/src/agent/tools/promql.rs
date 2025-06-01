//! PromQL Tool for Prometheus Queries
//! 
//! Allows agents to query Prometheus metrics for investigation.

use super::{ToolResult, ToolArgs, ToolError};
use anyhow::Result;
use reqwest::Client;
use rig::completion::ToolDefinition;
use rig::tool::Tool as RigTool;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// PromQL tool for querying Prometheus
#[derive(Clone)]
pub struct PromQLTool {
    prometheus_url: String,
    client: Client,
    auth_token: Option<String>,
    timeout: Duration,
}

impl PromQLTool {
    pub fn new(prometheus_url: String) -> Self {
        Self {
            prometheus_url,
            client: Client::new(),
            auth_token: None,
            timeout: Duration::from_secs(30),
        }
    }
    
    /// Set authentication token
    pub fn with_auth_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }
    
    /// Set query timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    /// Execute a PromQL query
    async fn query(&self, query: &str) -> Result<PrometheusResponse> {
        let url = format!("{}/api/v1/query", self.prometheus_url);
        
        let mut request = self.client
            .get(&url)
            .query(&[("query", query)])
            .timeout(self.timeout);
        
        // Add auth header if token is provided
        if let Some(token) = &self.auth_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Prometheus query failed: {}", error_text));
        }
        
        let result: PrometheusResponse = response.json().await?;
        Ok(result)
    }
    
    /// Execute a PromQL range query
    async fn query_range(&self, query: &str, start: &str, end: &str, step: &str) -> Result<PrometheusResponse> {
        let url = format!("{}/api/v1/query_range", self.prometheus_url);
        
        let mut request = self.client
            .get(&url)
            .query(&[
                ("query", query),
                ("start", start),
                ("end", end),
                ("step", step),
            ])
            .timeout(self.timeout);
        
        if let Some(token) = &self.auth_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Prometheus range query failed: {}", error_text));
        }
        
        let result: PrometheusResponse = response.json().await?;
        Ok(result)
    }
    
    /// Execute a PromQL range query
    async fn range_query(&self, query: &str, start: i64, end: i64, step: &str) -> Result<PrometheusResponse> {
        let url = format!("{}/api/v1/query_range", self.prometheus_url);
        
        let mut request = self.client
            .get(&url)
            .query(&[
                ("query", query),
                ("start", &start.to_string()),
                ("end", &end.to_string()),
                ("step", step),
            ])
            .timeout(self.timeout);
        
        // Add auth header if token is provided
        if let Some(token) = &self.auth_token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Prometheus query failed: {}", error_text));
        }
        
        let result: PrometheusResponse = response.json().await?;
        Ok(result)
    }
    
    /// Parse command to determine query type
    fn parse_command(&self, input: &str) -> Result<PromQLCommand> {
        // For now, we only support instant queries
        // TODO: Add support for range queries with time parameters
        Ok(PromQLCommand::InstantQuery(input.to_string()))
    }
    
    /// Validate if the query is safe to execute
    fn validate(&self, input: &str) -> Result<()> {
        // Basic validation - check for common injection attempts
        if input.contains(';') || input.contains("&&") || input.contains("||") {
            return Err(anyhow::anyhow!("Invalid characters in PromQL query"));
        }
        
        // Check query length
        if input.len() > 1000 {
            return Err(anyhow::anyhow!("Query too long (max 1000 characters)"));
        }
        
        Ok(())
    }
}

impl RigTool for PromQLTool {
    const NAME: &'static str = "promql";
    
    type Error = ToolError;
    type Args = ToolArgs;
    type Output = ToolResult;
    
    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Query Prometheus metrics using PromQL. Supports instant queries like \
                         'up{job=\"kubernetes-pods\"}' or 'rate(http_requests_total[5m])'. \
                         Returns metric values and labels.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The PromQL query to execute (e.g., 'rate(http_requests_total[5m])')"
                    }
                },
                "required": ["command"]
            }),
        }
    }
    
    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Validate the query
        self.validate(&args.command)
            .map_err(|e| ToolError::ValidationError(e.to_string()))?;
        
        // Execute the query
        match self.parse_command(&args.command) {
            Ok(PromQLCommand::InstantQuery(query)) => {
                match self.query(&query).await {
                    Ok(response) => {
                        let output = format_prometheus_response(&response);
                        Ok(ToolResult {
                            success: true,
                            output,
                            error: None,
                            metadata: Some(serde_json::to_value(&response).unwrap()),
                        })
                    }
                    Err(e) => Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(e.to_string()),
                        metadata: None,
                    }),
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
                metadata: None,
            }),
        }
    }
}

#[derive(Debug)]
enum PromQLCommand {
    InstantQuery(String),
    // Could add RangeQuery(query, start, end, step) in the future
}

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusResult {
    metric: serde_json::Value,
    value: Option<(f64, String)>,
    values: Option<Vec<(f64, String)>>,
}

/// Format Prometheus response for human-readable output
fn format_prometheus_response(response: &PrometheusResponse) -> String {
    let mut output = String::new();
    
    if response.data.result.is_empty() {
        return "No data found for the query".to_string();
    }
    
    for result in &response.data.result {
        // Format metric labels
        if let Some(metric_obj) = result.metric.as_object() {
            if !metric_obj.is_empty() {
                output.push_str("Metric: {");
                let labels: Vec<String> = metric_obj.iter()
                    .map(|(k, v)| format!("{}=\"{}\"", k, v.as_str().unwrap_or("")))
                    .collect();
                output.push_str(&labels.join(", "));
                output.push_str("}\n");
            }
        }
        
        // Format value(s)
        if let Some((timestamp, value)) = &result.value {
            output.push_str(&format!("Value: {} @ {}\n", value, timestamp));
        }
        
        if let Some(values) = &result.values {
            output.push_str("Values:\n");
            for (timestamp, value) in values {
                output.push_str(&format!("  {} @ {}\n", value, timestamp));
            }
        }
        
        output.push('\n');
    }
    
    output
} 