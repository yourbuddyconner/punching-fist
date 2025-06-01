//! Example showing how to use the real agent runtime for investigations
//! 
//! This demonstrates how the agent runtime executes real LLM-powered investigations
//! using the integrated tools.

use anyhow::Result;
use punching_fist_operator::agent::{
    runtime::AgentRuntime,
    provider::LLMConfig,
};
use kube::Client;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🤖 Agent Runtime Example - Real LLM Investigation\n");
    
    // Configure LLM provider
    let llm_config = LLMConfig {
        provider: "mock".to_string(), // Use "anthropic" or "openai" with API key
        model: "claude-3-5-sonnet".to_string(),
        api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
        temperature: Some(0.7),
        max_tokens: Some(15), // This is max iterations, not tokens
        timeout_seconds: Some(300),
        endpoint: None,
    };
    
    // Create agent runtime
    let mut runtime = AgentRuntime::new(llm_config)?;
    println!("✅ Created agent runtime");
    
    // Configure runtime with Kubernetes client if available
    match Client::try_default().await {
        Ok(k8s_client) => {
            println!("✅ Connected to Kubernetes cluster");
            runtime = runtime
                .with_k8s_client(k8s_client)
                .with_prometheus_endpoint("http://prometheus:9090".to_string());
            println!("✅ Configured runtime with Kubernetes client and tools");
        }
        Err(e) => {
            println!("⚠️  No Kubernetes connection: {}", e);
            println!("   Running with limited functionality");
        }
    }
    
    // Example 1: Pod crash investigation
    println!("\n🔍 Example 1: Investigating Pod Crash");
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), "PodCrashLooping".to_string());
    context.insert("pod".to_string(), "my-app-5f7b8c9d4-xyz123".to_string());
    context.insert("namespace".to_string(), "default".to_string());
    context.insert("container".to_string(), "app".to_string());
    context.insert("restartCount".to_string(), "5".to_string());
    
    let result = runtime.investigate(
        "Investigate why the pod is crash looping and provide recommendations",
        context,
    ).await?;
    
    println!("\n📊 Investigation Results:");
    println!("Summary: {}", result.summary);
    
    if let Some(root_cause) = &result.root_cause {
        println!("\n🎯 Root Cause: {}", root_cause);
    }
    
    if !result.findings.is_empty() {
        println!("\n🔍 Findings:");
        for finding in &result.findings {
            println!("  - [{:?}] {}: {}", 
                finding.severity, 
                finding.category, 
                finding.description
            );
        }
    }
    
    if !result.actions_taken.is_empty() {
        println!("\n🛠️  Actions Taken:");
        for action in &result.actions_taken {
            println!("  - {} {}: {}", 
                action.tool, 
                if action.success { "✓" } else { "✗" },
                action.command
            );
            if !action.output_summary.is_empty() {
                println!("    → {}", action.output_summary);
            }
        }
    } else {
        println!("\n📝 Note: Tool execution details are handled internally by Rig's agent system");
    }
    
    if !result.recommendations.is_empty() {
        println!("\n💡 Recommendations:");
        for rec in &result.recommendations {
            println!("  {}. {} (Risk: {:?})", 
                rec.priority, 
                rec.action,
                rec.risk_level
            );
            println!("     Rationale: {}", rec.rationale);
        }
    }
    
    if result.can_auto_fix {
        println!("\n🔧 Auto-fix available!");
        if let Some(fix_cmd) = &result.fix_command {
            println!("   Command: {}", fix_cmd);
        }
    }
    
    println!("\nConfidence: {:.0}%", result.confidence * 100.0);
    
    // Example 2: High CPU investigation
    println!("\n\n🔍 Example 2: Investigating High CPU Usage");
    let mut context2 = HashMap::new();
    context2.insert("alert_name".to_string(), "HighCPUUsage".to_string());
    context2.insert("service".to_string(), "api-gateway".to_string());
    context2.insert("namespace".to_string(), "production".to_string());
    context2.insert("cpu_usage".to_string(), "95%".to_string());
    context2.insert("duration".to_string(), "10m".to_string());
    
    let result2 = runtime.investigate(
        "Investigate high CPU usage and determine if scaling is needed",
        context2,
    ).await?;
    
    println!("\n📊 Investigation Results:");
    println!("Summary: {}", result2.summary);
    
    println!("\n✅ Real agent runtime demonstration complete!");
    println!("\n💡 Key Features:");
    println!("  - Uses Rig's agent system with automatic tool handling");
    println!("  - Tools are registered with the agent builder");
    println!("  - LLM automatically decides which tools to use");
    println!("  - Tool execution is handled internally by Rig");
    println!("  - Structured result parsing from LLM response");
    println!("  - Confidence scoring via separate LLM call");
    
    Ok(())
} 