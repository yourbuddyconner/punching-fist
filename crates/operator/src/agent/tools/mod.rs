//! Agent Tools Module
//! 
//! Provides tools that LLM agents can use to investigate alerts and perform actions.
//! All tools implement Rig's Tool trait for seamless integration with LLM agents.

pub mod kubectl;
pub mod promql;
pub mod curl;
pub mod script;

use serde::{Deserialize, Serialize};

/// Result from tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

// Re-export tool implementations
pub use kubectl::KubectlTool;
pub use promql::PromQLTool;
pub use curl::CurlTool;
pub use script::ScriptTool;

/// Arguments for tool execution (used by all tools)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolArgs {
    pub command: String,
}

/// Error type for tool execution
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool execution failed: {0}")]
    ExecutionError(String),
    
    #[error("Validation failed: {0}")]
    ValidationError(String),
    
    #[error("Internal error: {0}")]
    InternalError(#[from] anyhow::Error),
}

// The actual Rig Tool trait implementations are in each tool's module
// This keeps the code organized and avoids async_trait conflicts 