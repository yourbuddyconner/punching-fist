//! Test the agent runtime functionality
//! 
//! Run with: cargo run --example test_agent

use punching_fist_operator::agent::{AgentRuntime, LLMConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("punching_fist_operator=debug,info")
        .init();

    // Test with mock provider first
    println!("=== Testing with Mock Provider ===\n");
    test_mock_provider().await?;
    
    // Test with Anthropic if API key is available
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        println!("\n=== Testing with Anthropic Provider ===\n");
        test_anthropic_provider().await?;
    } else {
        println!("\n=== Skipping Anthropic test (no API key) ===");
        println!("Set ANTHROPIC_API_KEY to test with Claude");
    }
    
    // Test with OpenAI if API key is available
    if std::env::var("OPENAI_API_KEY").is_ok() {
        println!("\n=== Testing with OpenAI Provider ===\n");
        test_openai_provider().await?;
    } else {
        println!("\n=== Skipping OpenAI test (no API key) ===");
        println!("Set OPENAI_API_KEY to test with GPT models");
    }

    Ok(())
}

async fn test_mock_provider() -> Result<(), Box<dyn std::error::Error>> {
    let llm_config = LLMConfig {
        provider: "mock".to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), "PodCrashLooping".to_string());
    context.insert("pod".to_string(), "test-pod".to_string());
    
    let result = agent_runtime.investigate(
        "Pod is crash looping, investigate the issue",
        context
    ).await?;
    
    println!("Summary: {}", result.summary);
    println!("Confidence: {:.2}", result.confidence);
    
    Ok(())
}

async fn test_anthropic_provider() -> Result<(), Box<dyn std::error::Error>> {
    let llm_config = LLMConfig {
        provider: "anthropic".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), "HighCPUUsage".to_string());
    context.insert("service".to_string(), "api-service".to_string());
    context.insert("cpu_percent".to_string(), "95".to_string());
    
    let result = agent_runtime.investigate(
        "Service is experiencing high CPU usage (95%), investigate and provide recommendations",
        context
    ).await?;
    
    println!("Summary: {}", result.summary);
    if let Some(root_cause) = &result.root_cause {
        println!("Root Cause: {}", root_cause);
    }
    
    Ok(())
}

async fn test_openai_provider() -> Result<(), Box<dyn std::error::Error>> {
    let llm_config = LLMConfig {
        provider: "openai".to_string(),
        model: "gpt-4".to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), "ServiceUnavailable".to_string());
    context.insert("service".to_string(), "checkout-service".to_string());
    context.insert("error".to_string(), "Connection timeout".to_string());
    
    let result = agent_runtime.investigate(
        "Checkout service is unavailable with connection timeout errors",
        context
    ).await?;
    
    println!("Summary: {}", result.summary);
    if !result.recommendations.is_empty() {
        println!("First Recommendation: {}", result.recommendations[0].action);
    }
    
    Ok(())
} 