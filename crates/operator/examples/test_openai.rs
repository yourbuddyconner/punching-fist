//! Test the agent runtime with real OpenAI
//! 
//! Run with: OPENAI_API_KEY=your-key cargo run --example test_openai

use punching_fist_operator::agent::{AgentRuntime, LLMConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("punching_fist_operator=info")
        .init();

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set");
        eprintln!("Usage: OPENAI_API_KEY=your-key cargo run --example test_openai");
        return Ok(());
    }
    
    println!("=== Testing with Real OpenAI ===\n");
    
    // Create agent runtime with OpenAI provider
    let llm_config = LLMConfig {
        provider: "openai".to_string(),
        model: "gpt-4".to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    
    // Create a simple test scenario
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), "ServiceUnavailable".to_string());
    context.insert("service".to_string(), "payment-api".to_string());
    context.insert("namespace".to_string(), "production".to_string());
    context.insert("severity".to_string(), "critical".to_string());
    context.insert("message".to_string(), "Service payment-api is returning 503 errors".to_string());
    
    println!("Investigating service unavailability issue...\n");
    
    let result = agent_runtime.investigate(
        "The payment-api service is returning 503 Service Unavailable errors. Please investigate what might be causing this issue.",
        context
    ).await?;
    
    println!("Investigation Summary:\n{}\n", result.summary);
    println!("Confidence: {:.2}", result.confidence);
    
    if let Some(root_cause) = &result.root_cause {
        println!("\nRoot Cause:\n{}", root_cause);
    }
    
    if !result.recommendations.is_empty() {
        println!("\nRecommendations:");
        for (i, rec) in result.recommendations.iter().enumerate() {
            println!("{}. {} (Risk: {:?})", i + 1, rec.action, rec.risk_level);
            println!("   Rationale: {}", rec.rationale);
        }
    }
    
    println!("\n=== Full Report ===\n{}", result.format_report());
    
    Ok(())
} 