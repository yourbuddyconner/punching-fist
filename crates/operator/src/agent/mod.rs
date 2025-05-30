//! LLM Agent Runtime Module
//! 
//! This module provides the LLM-powered agent execution runtime for intelligent
//! alert investigation and automated remediation.

pub mod provider;
pub mod runtime;
pub mod tools;
pub mod safety;
pub mod templates;
pub mod result;

pub use provider::{LLMProvider, LLMConfig};
pub use runtime::AgentRuntime;
pub use result::{AgentResult, Finding};
pub use tools::{Tool, ToolResult}; 