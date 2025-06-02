//! Test the agent runtime with Anthropic Claude
//! 
//! Run with: ANTHROPIC_API_KEY=your-key cargo run --example test_anthropic

use punching_fist_operator::agent::{AgentRuntime, LLMConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("punching_fist_operator=info")
        .init();

    println!("=== Testing with Anthropic Claude ===\n");
    
    // Check if API key is available
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        println!("Note: Running in mock mode. To use real Anthropic API:");
        println!("1. Set ANTHROPIC_API_KEY environment variable");
        println!("2. Run: ANTHROPIC_API_KEY=your-key cargo run --example test_anthropic\n");
    } else {
        println!("Using real Anthropic API (Claude 3.5 Sonnet)\n");
    }
    
    // Create agent runtime with Anthropic provider
    let llm_config = LLMConfig {
        provider: "anthropic".to_string(),
        model: "claude-3-5-sonnet".to_string(),
        api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    
    // Test pod crash investigation
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), "PodCrashLooping".to_string());
    context.insert("pod".to_string(), "api-server-xyz".to_string());
    context.insert("namespace".to_string(), "production".to_string());
    context.insert("severity".to_string(), "critical".to_string());
    context.insert("error".to_string(), "Container terminated with exit code 137".to_string());
    
    println!("Scenario: Pod crash investigation\n");
    println!("Alert: PodCrashLooping");
    println!("Pod: api-server-xyz");
    println!("Error: Container terminated with exit code 137\n");
    
    let result = agent_runtime.investigate(
        "A production pod is crash looping with exit code 137. Investigate the issue and provide recommendations.",
        context
    ).await?;
    
    println!("Investigation Results:");
    println!("====================\n");
    println!("Summary: {}", result.summary);
    println!("Confidence: {:.2}", result.confidence);
    
    if let Some(root_cause) = &result.root_cause {
        println!("\nRoot Cause: {}", root_cause);
    }
    
    if !result.recommendations.is_empty() {
        println!("\nRecommendations:");
        for (i, rec) in result.recommendations.iter().enumerate() {
            println!("{}. {} (Risk: {:?})", i + 1, rec.action, rec.risk_level);
            println!("   Rationale: {}", rec.rationale);
        }
    }
    
    // Test another scenario
    println!("\n\n=== Second Scenario: Service Degradation ===\n");
    
    let mut context2 = HashMap::new();
    context2.insert("alert_name".to_string(), "HighResponseTime".to_string());
    context2.insert("service".to_string(), "checkout-service".to_string());
    context2.insert("namespace".to_string(), "production".to_string());
    context2.insert("severity".to_string(), "warning".to_string());
    context2.insert("p95_latency".to_string(), "2500ms".to_string());
    context2.insert("normal_p95".to_string(), "200ms".to_string());
    
    let result2 = agent_runtime.investigate(
        "The checkout service is experiencing high response times (p95: 2500ms vs normal 200ms). Investigate potential causes.",
        context2
    ).await?;
    
    println!("Summary: {}", result2.summary);
    
    if let Some(root_cause) = &result2.root_cause {
        println!("\nRoot Cause: {}", root_cause);
    }
    
    println!("\n\nSupported models:");
    println!("- claude-3-5-sonnet (default, most capable)");
    println!("- claude-3-opus (most powerful, slower)");
    println!("- claude-3-sonnet (balanced)");
    println!("- claude-3-haiku (fastest, most cost-effective)");
    
    Ok(())
} 