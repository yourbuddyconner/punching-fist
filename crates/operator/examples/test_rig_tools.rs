//! Example showing how to use Rig-integrated tools with LLM agents
//! 
//! This demonstrates how our tools implement Rig's Tool trait and can be used
//! with Rig's agent system.

use anyhow::Result;
use punching_fist_operator::agent::tools::{
    kubectl::KubectlTool,
    promql::PromQLTool,
    curl::CurlTool,
    script::ScriptTool,
    ToolArgs,
};
use rig::tool::Tool as RigTool;
use kube::Client;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🛠️  Rig Tools Integration Example\n");
    
    // Create Kubernetes client
    let k8s_client = match Client::try_default().await {
        Ok(client) => client,
        Err(_) => {
            println!("⚠️  No Kubernetes connection - using mock demonstrations");
            println!("   Connect to a Kubernetes cluster to see full functionality\n");
            return demonstrate_tool_definitions().await;
        }
    };
    
    // Create our tools
    let kubectl = KubectlTool::new(k8s_client.clone());
    let promql = PromQLTool::new("http://prometheus:9090".to_string());
    let curl = CurlTool::new();
    let script = ScriptTool::new();
    
    println!("✅ Created tools that implement Rig's Tool trait");
    
    // Demonstrate tool definitions
    println!("\n📋 Tool Definitions:");
    
    let kubectl_def = kubectl.definition("Investigate pod issues".to_string()).await;
    println!("\nKubectl Tool:");
    println!("  Name: {}", kubectl_def.name);
    println!("  Description: {}", kubectl_def.description);
    println!("  Parameters: {}", serde_json::to_string_pretty(&kubectl_def.parameters)?);
    
    let promql_def = promql.definition("Query metrics".to_string()).await;
    println!("\nPromQL Tool:");
    println!("  Name: {}", promql_def.name);
    println!("  Description: {}", promql_def.description);
    println!("  Parameters: {}", serde_json::to_string_pretty(&promql_def.parameters)?);
    
    // Demonstrate tool execution
    println!("\n🔧 Tool Execution Examples:");
    
    // Kubectl example
    let kubectl_args = ToolArgs {
        command: "get pods -n kube-system".to_string(),
    };
    
    match kubectl.call(kubectl_args).await {
        Ok(result) => {
            println!("\nKubectl execution:");
            println!("  Success: {}", result.success);
            if result.success {
                println!("  Output preview: {}...", 
                    result.output.lines().take(3).collect::<Vec<_>>().join("\n  "));
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    
    // PromQL example
    let promql_args = ToolArgs {
        command: "up{job=\"kubernetes-pods\"}".to_string(),
    };
    
    match promql.call(promql_args).await {
        Ok(result) => {
            println!("\nPromQL execution:");
            println!("  Success: {}", result.success);
            if let Some(error) = result.error {
                println!("  Error: {}", error);
            }
        }
        Err(e) => println!("  Error: {}", e),
    }
    
    println!("\n✅ Rig tools integration complete!");
    println!("\n💡 These tools can be used with Rig agents to enable LLM-powered");
    println!("   investigation and remediation workflows!");
    
    Ok(())
}

async fn demonstrate_tool_definitions() -> Result<()> {
    use punching_fist_operator::agent::tools::ToolResult;
    
    println!("📋 Tool Architecture:");
    println!("\nOur tools implement Rig's Tool trait with:");
    println!("  - const NAME: Tool identifier");
    println!("  - type Args/Output/Error: Type-safe parameters");
    println!("  - definition(): OpenAPI-style parameter schema for LLMs");
    println!("  - call(): Async execution with validation");
    
    println!("\nBenefits of using Rig's Tool trait:");
    println!("  - Type-safe integration with LLM agents");
    println!("  - Automatic tool discovery and registration");
    println!("  - Built-in parameter validation");
    println!("  - Consistent error handling");
    
    println!("\n✅ All our tools (kubectl, promql, curl, script) implement Rig's Tool trait!");
    
    Ok(())
} 