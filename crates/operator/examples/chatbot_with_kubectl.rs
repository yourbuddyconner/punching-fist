//! Example showing how to use the Chatbot agent with kubectl tool
//! 
//! This demonstrates using the chatbot with automatic kubectl configuration

use anyhow::Result;
use punching_fist_operator::agent::{
    runtime::{AgentRuntime, ToolType},
    chatbot::ChatbotAgent,
    behavior::{AgentBehavior, AgentInput, AgentOutput},
    provider::LLMConfig,
    tools::kubectl::KubectlTool,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("🤖 Chatbot with Kubectl Tool Example\n");
    
    // Configure LLM (using mock for demonstration)
    let llm_config = LLMConfig {
        provider: "mock".to_string(),
        model: "gpt-4".to_string(),
        api_key: None,
        endpoint: None,
        temperature: Some(0.7),
        max_tokens: Some(500),
        timeout_seconds: Some(30),
    };
    
    // Create agent runtime
    let mut runtime = AgentRuntime::new(llm_config)?;
    
    // Try to create kubectl tool with automatic configuration
    println!("🔧 Setting up kubectl tool with automatic configuration...");
    match KubectlTool::infer().await {
        Ok(kubectl_tool) => {
            println!("✅ Successfully created kubectl tool using inferred config");
            println!("   (Using kubeconfig or in-cluster service account)");
            
            // Add the kubectl tool to runtime
            runtime.add_tool("kubectl".to_string(), kubectl_tool);
            
            // Also add other tools if needed
            runtime.add_tool("promql".to_string(), 
                punching_fist_operator::agent::tools::promql::PromQLTool::new("http://prometheus:9090".to_string()));
        }
        Err(e) => {
            println!("⚠️  Could not infer Kubernetes config: {}", e);
            println!("   Chatbot will run without kubectl tool");
        }
    }
    
    // Create chatbot agent
    let chatbot = runtime.get_chatbot_agent();
    println!("\n💬 Chatbot is ready! It has access to:");
    println!("   - kubectl commands (if Kubernetes is available)");
    println!("   - PromQL queries");
    println!("   - General Kubernetes knowledge\n");
    
    // Example conversation
    let chat_examples = vec![
        "Can you check what pods are running in the default namespace?",
        "Show me the CPU usage of pods in kube-system",
        "Why might a pod be in CrashLoopBackOff state?",
        "How can I debug a pod that's not starting?",
    ];
    
    for (i, message) in chat_examples.iter().enumerate() {
        println!("👤 User: {}", message);
        
        let input = AgentInput::ChatMessage {
            content: message.to_string(),
            history: vec![], // In a real app, you'd maintain conversation history
            session_id: Some("example-session".to_string()),
            user_id: Some("example-user".to_string()),
        };
        
        // Get chatbot response
        match runtime.execute(&chatbot, input).await {
            Ok(AgentOutput::ChatResponse { 
                message, 
                suggested_actions,
                tool_calls_this_turn,
                .. 
            }) => {
                println!("🤖 Assistant: {}", message);
                
                if let Some(tool_calls) = tool_calls_this_turn {
                    println!("   📊 Tools used: {:?}", tool_calls.len());
                }
                
                if let Some(suggestions) = suggested_actions {
                    println!("   💡 Suggested actions:");
                    for suggestion in suggestions {
                        println!("      - {}", suggestion);
                    }
                }
            }
            Ok(other) => {
                println!("❌ Unexpected response type: {:?}", other);
            }
            Err(e) => {
                println!("❌ Error: {}", e);
            }
        }
        
        if i < chat_examples.len() - 1 {
            println!(); // Add spacing between conversations
        }
    }
    
    println!("\n✅ Chatbot example complete!");
    println!("\n💡 Key Features Demonstrated:");
    println!("   - Automatic kubectl configuration using infer()");
    println!("   - Chatbot with integrated Kubernetes tools");
    println!("   - Context-aware responses and suggestions");
    println!("   - Tool execution through natural language");
    
    Ok(())
} 