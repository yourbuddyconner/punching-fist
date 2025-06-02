//! LLM Agent Runtime Module
//! 
//! This module provides the LLM-powered agent execution runtime for intelligent
//! alert investigation and automated remediation.

pub mod behavior;
pub mod chatbot;
pub mod investigator;
pub mod provider;
pub mod runtime;
pub mod tools;
pub mod safety;
pub mod templates;
pub mod result;

pub use behavior::{AgentBehavior, AgentInput, AgentOutput, AgentContext, AgentBehaviorConfig};
pub use chatbot::ChatbotAgent;
pub use investigator::InvestigatorAgent;
pub use provider::{LLMProvider, LLMConfig};
pub use runtime::{AgentRuntime, ToolType};
pub use result::{AgentResult, Finding};
pub use tools::{ToolResult, ToolArgs, ToolError}; 