//! CLI tool for testing agent functionality in isolation
//! 
//! Run with: cargo run --bin test-agent -- [OPTIONS]

use punching_fist_operator::agent::{
    AgentRuntime, LLMConfig, AgentInput, AgentOutput
};
use punching_fist_operator::agent::tools::{PromQLTool, CurlTool, ScriptTool, KubectlTool};
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use anyhow::Result;
use clap::{Parser, Subcommand};
use rig::completion::Message;
use chrono::Utc;

#[derive(Parser)]
#[command(author, version, about = "Test agent functionality in isolation", long_about = None)]
struct Cli {
    /// Log level (debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
    
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Test with mock provider (no API key required)
    Mock {
        /// Investigation goal
        #[arg(short, long, default_value = "Investigate the alert")]
        goal: String,
        
        /// Alert name
        #[arg(short, long, default_value = "TestAlert")]
        alert: String,
    },
    
    /// Test with Anthropic/Claude
    Anthropic {
        /// Investigation goal
        #[arg(short, long, default_value = "Investigate the alert")]
        goal: String,
        
        /// Model to use
        #[arg(short, long, default_value = "claude-3-5-sonnet")]
        model: String,
        
        /// API key (defaults to ANTHROPIC_API_KEY env var)
        #[arg(long)]
        api_key: Option<String>,
        
        /// Enable tools
        #[arg(short, long)]
        tools: bool,
    },
    
    /// Test with OpenAI
    OpenAI {
        /// Investigation goal
        #[arg(short, long, default_value = "Investigate the alert")]
        goal: String,
        
        /// Model to use
        #[arg(short, long, default_value = "gpt-4")]
        model: String,
        
        /// API key (defaults to OPENAI_API_KEY env var)
        #[arg(long)]
        api_key: Option<String>,
        
        /// Enable tools
        #[arg(short, long)]
        tools: bool,
    },
    
    /// Interactive mode - prompt for all inputs
    Interactive,
    
    /// Test specific scenarios
    Scenario {
        /// Scenario name (pod-crash, high-cpu, memory-leak, network-issue)
        #[arg(short, long)]
        name: String,
        
        /// Provider to use (mock, anthropic, openai)
        #[arg(short, long, default_value = "mock")]
        provider: String,
    },
    
    /// Interactive chatbot mode
    Chatbot {
        /// Provider to use (mock, anthropic, openai)
        #[arg(short, long, default_value = "anthropic")]
        provider: String,
        
        /// Model to use (if not mock)
        #[arg(short, long)]
        model: Option<String>,
    },
    
    /// Test the new investigator agent
    Investigate {
        /// Provider to use (mock, anthropic, openai)
        #[arg(short, long, default_value = "anthropic")]
        provider: String,
        
        /// Enable human approval simulation
        #[arg(short, long)]
        approval: bool,
    },
}

/// Helper function to get cluster context information
async fn get_cluster_context() -> Option<String> {
    match KubectlTool::infer().await {
        Ok(kubectl_tool) => {
            match kubectl_tool.get_cluster_context().await {
                Ok(context) => Some(context),
                Err(e) => {
                    eprintln!("Failed to get cluster context: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("kubectl tool not available: {}", e);
            None
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Show current working directory for debugging
    let cwd = env::current_dir()?;
    eprintln!("Current working directory: {}", cwd.display());
    
    // Load .env file if it exists
    match dotenvy::dotenv() {
        Ok(path) => {
            eprintln!("Loaded .env from: {}", path.display());
        }
        Err(e) => {
            if e.not_found() {
                eprintln!("No .env file found in current directory");
                
                // Try to find .env in parent directories (useful when running from subdirectories)
                let mut search_dir = cwd.clone();
                let mut found = false;
                
                for _ in 0..5 {  // Search up to 5 parent directories
                    if let Some(parent) = search_dir.parent() {
                        search_dir = parent.to_path_buf();
                        let env_path = search_dir.join(".env");
                        if env_path.exists() {
                            eprintln!("Found .env at: {}", env_path.display());
                            if let Err(e) = dotenvy::from_path(&env_path) {
                                eprintln!("Warning: Failed to load .env from {}: {}", env_path.display(), e);
                            } else {
                                eprintln!("Successfully loaded .env from: {}", env_path.display());
                                found = true;
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
                
                if !found {
                    eprintln!("No .env file found in current or parent directories");
                    eprintln!("Using system environment variables only");
                }
            } else {
                eprintln!("Warning: Error loading .env file: {}", e);
            }
        }
    }
    
    // Show if key environment variables are set (without revealing values)
    if env::var("ANTHROPIC_API_KEY").is_ok() {
        eprintln!("ANTHROPIC_API_KEY is set");
    } else {
        eprintln!("ANTHROPIC_API_KEY is NOT set");
    }
    
    if env::var("OPENAI_API_KEY").is_ok() {
        eprintln!("OPENAI_API_KEY is set");
    } else {
        eprintln!("OPENAI_API_KEY is NOT set");
    }
    
    eprintln!(); // Add blank line before starting the CLI
    
    let cli = Cli::parse();
    
    // Initialize logging
    let log_filter = format!("punching_fist_operator={},info", cli.log_level);
    tracing_subscriber::fmt()
        .with_env_filter(log_filter)
        .init();
    
    match cli.command {
        Commands::Mock { goal, alert } => {
            test_mock_provider(&goal, &alert).await?;
        }
        Commands::Anthropic { goal, model, api_key, tools } => {
            test_anthropic_provider(&goal, &model, api_key, tools).await?;
        }
        Commands::OpenAI { goal, model, api_key, tools } => {
            test_openai_provider(&goal, &model, api_key, tools).await?;
        }
        Commands::Interactive => {
            run_interactive_mode().await?;
        }
        Commands::Scenario { name, provider } => {
            run_scenario(&name, &provider).await?;
        }
        Commands::Chatbot { provider, model } => {
            run_chatbot_mode(&provider, model).await?;
        }
        Commands::Investigate { provider, approval } => {
            run_investigator_mode_interactive(&provider, approval).await?;
        }
    }
    
    Ok(())
}

async fn test_mock_provider(goal: &str, alert_name: &str) -> Result<()> {
    println!("=== Testing with Mock Provider ===");
    println!("Alert: {}", alert_name);
    println!("Goal: {}", goal);
    println!();
    
    let llm_config = LLMConfig {
        provider: "mock".to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    
    let mut context = HashMap::new();
    context.insert("alert_name".to_string(), alert_name.to_string());
    
    let result = agent_runtime.investigate(goal, context).await?;
    
    print_results(&result);
    
    Ok(())
}

async fn test_anthropic_provider(goal: &str, model: &str, api_key: Option<String>, enable_tools: bool) -> Result<()> {
    println!("=== Testing with Anthropic Provider ===");
    println!("Model: {}", model);
    println!("Goal: {}", goal);
    println!("Tools enabled: {}", enable_tools);
    println!();
    
    // Check for API key
    let api_key = api_key.or_else(|| env::var("ANTHROPIC_API_KEY").ok());
    if api_key.is_none() {
        eprintln!("Error: No API key provided. Set ANTHROPIC_API_KEY or use --api-key");
        return Ok(());
    }
    
    let llm_config = LLMConfig {
        provider: "anthropic".to_string(),
        model: model.to_string(),
        api_key,
        ..Default::default()
    };
    
    let mut agent_runtime = AgentRuntime::new(llm_config)?;
    
    // Add tools if enabled
    if enable_tools {
        println!("Adding tools to agent runtime...");
        // Get Prometheus endpoint from env or use default
        let prometheus_endpoint = env::var("PROMETHEUS_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:9090".to_string());
        
        // Try to add kubectl tool with automatic configuration
        match KubectlTool::infer().await {
            Ok(kubectl_tool) => {
                // Get cluster context information
                match kubectl_tool.get_cluster_context().await {
                    Ok(context) => {
                        println!("✅ kubectl tool initialized with inferred configuration");
                        println!("\n📋 Cluster Context:");
                        for line in context.lines() {
                            println!("   {}", line);
                        }
                        println!();
                    }
                    Err(e) => {
                        println!("✅ kubectl tool initialized (context fetch failed: {})", e);
                    }
                }
                agent_runtime.add_tool("kubectl".to_string(), kubectl_tool);
            }
            Err(e) => {
                println!("⚠️  kubectl tool not available: {}", e);
            }
        }
        
        agent_runtime.add_tool("promql".to_string(), PromQLTool::new(prometheus_endpoint));
        agent_runtime.add_tool("curl".to_string(), CurlTool::new());
        agent_runtime.add_tool("script".to_string(), ScriptTool::new());
    }
    
    let context = HashMap::new();
    let result = agent_runtime.investigate(goal, context).await?;
    
    print_results(&result);
    
    Ok(())
}

async fn test_openai_provider(goal: &str, model: &str, api_key: Option<String>, enable_tools: bool) -> Result<()> {
    println!("=== Testing with OpenAI Provider ===");
    println!("Model: {}", model);
    println!("Goal: {}", goal);
    println!("Tools enabled: {}", enable_tools);
    println!();
    
    // Check for API key
    let api_key = api_key.or_else(|| env::var("OPENAI_API_KEY").ok());
    if api_key.is_none() {
        eprintln!("Error: No API key provided. Set OPENAI_API_KEY or use --api-key");
        return Ok(());
    }
    
    let llm_config = LLMConfig {
        provider: "openai".to_string(),
        model: model.to_string(),
        api_key,
        ..Default::default()
    };
    
    let mut agent_runtime = AgentRuntime::new(llm_config)?;
    
    // Add tools if enabled
    if enable_tools {
        println!("Adding tools to agent runtime...");
        // Get Prometheus endpoint from env or use default
        let prometheus_endpoint = env::var("PROMETHEUS_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:9090".to_string());
        
        // Try to add kubectl tool with automatic configuration
        match KubectlTool::infer().await {
            Ok(kubectl_tool) => {
                // Get cluster context information
                match kubectl_tool.get_cluster_context().await {
                    Ok(context) => {
                        println!("✅ kubectl tool initialized with inferred configuration");
                        println!("\n📋 Cluster Context:");
                        for line in context.lines() {
                            println!("   {}", line);
                        }
                        println!();
                    }
                    Err(e) => {
                        println!("✅ kubectl tool initialized (context fetch failed: {})", e);
                    }
                }
                agent_runtime.add_tool("kubectl".to_string(), kubectl_tool);
            }
            Err(e) => {
                println!("⚠️  kubectl tool not available: {}", e);
            }
        }
        
        agent_runtime.add_tool("promql".to_string(), PromQLTool::new(prometheus_endpoint));
        agent_runtime.add_tool("curl".to_string(), CurlTool::new());
        agent_runtime.add_tool("script".to_string(), ScriptTool::new());
    }
    
    let context = HashMap::new();
    let result = agent_runtime.investigate(goal, context).await?;
    
    print_results(&result);
    
    Ok(())
}

async fn run_interactive_mode() -> Result<()> {
    println!("=== Interactive Agent Testing ===");
    println!();
    
    // Get default provider from env
    let default_provider = env::var("DEFAULT_PROVIDER").unwrap_or_else(|_| "mock".to_string());
    
    // Get provider
    print!("Select provider (mock/anthropic/openai) [{}]: ", default_provider);
    io::stdout().flush()?;
    let mut provider = String::new();
    io::stdin().read_line(&mut provider)?;
    let provider = provider.trim();
    let provider = if provider.is_empty() { &default_provider } else { provider };
    
    // Get model if not mock
    let model = if provider != "mock" {
        let default_model = match provider {
            "anthropic" => env::var("DEFAULT_ANTHROPIC_MODEL")
                .unwrap_or_else(|_| "claude-3-5-sonnet".to_string()),
            "openai" => env::var("DEFAULT_OPENAI_MODEL")
                .unwrap_or_else(|_| "gpt-4".to_string()),
            _ => "default".to_string(),
        };
        
        print!("Enter model name [{}]: ", default_model);
        io::stdout().flush()?;
        let mut model_input = String::new();
        io::stdin().read_line(&mut model_input)?;
        let model_input = model_input.trim();
        if model_input.is_empty() {
            default_model
        } else {
            model_input.to_string()
        }
    } else {
        "mock".to_string()
    };
    
    // Get goal
    print!("Enter investigation goal: ");
    io::stdout().flush()?;
    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim();
    
    let goal = if goal.is_empty() {
        println!("No goal provided, using default investigation");
        "General cluster health check".to_string()
    } else {
        goal.to_string()
    };
    
    // Get context
    println!("Enter context key-value pairs (empty line to finish):");
    let mut context = HashMap::new();
    loop {
        print!("Key (or empty to finish): ");
        io::stdout().flush()?;
        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim();
        
        if key.is_empty() {
            break;
        }
        
        print!("Value: ");
        io::stdout().flush()?;
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;
        let value = value.trim();
        
        context.insert(key.to_string(), value.to_string());
    }
    
    // Create and run agent
    println!("\nRunning investigation...\n");
    
    let llm_config = LLMConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    let result = agent_runtime.investigate(&goal, context).await?;
    
    print_results(&result);
    
    Ok(())
}

async fn run_scenario(scenario: &str, provider: &str) -> Result<()> {
    println!("=== Running Scenario: {} ===", scenario);
    println!("Provider: {}", provider);
    println!();
    
    let (goal, context) = match scenario {
        "pod-crash" => {
            let mut ctx = HashMap::new();
            ctx.insert("alert_name".to_string(), "PodCrashLooping".to_string());
            ctx.insert("namespace".to_string(), "default".to_string());
            ctx.insert("pod".to_string(), "api-server-7f9b8c6d5-xk2lm".to_string());
            ctx.insert("container".to_string(), "api".to_string());
            ctx.insert("restart_count".to_string(), "15".to_string());
            ctx.insert("exit_code".to_string(), "137".to_string());
            
            ("Pod is crash looping with exit code 137. Investigate the root cause and provide remediation steps.", ctx)
        }
        "high-cpu" => {
            let mut ctx = HashMap::new();
            ctx.insert("alert_name".to_string(), "HighCPUUsage".to_string());
            ctx.insert("namespace".to_string(), "production".to_string());
            ctx.insert("service".to_string(), "payment-processor".to_string());
            ctx.insert("cpu_percent".to_string(), "98".to_string());
            ctx.insert("duration".to_string(), "15m".to_string());
            
            ("Service is experiencing sustained high CPU usage at 98%. Investigate whether this is legitimate load or a performance issue.", ctx)
        }
        "memory-leak" => {
            let mut ctx = HashMap::new();
            ctx.insert("alert_name".to_string(), "MemoryLeak".to_string());
            ctx.insert("namespace".to_string(), "staging".to_string());
            ctx.insert("deployment".to_string(), "user-service".to_string());
            ctx.insert("memory_growth_rate".to_string(), "50MB/hour".to_string());
            ctx.insert("current_usage".to_string(), "850MB".to_string());
            ctx.insert("limit".to_string(), "1GB".to_string());
            
            ("Service shows signs of a memory leak with consistent growth. Investigate and recommend fixes.", ctx)
        }
        "network-issue" => {
            let mut ctx = HashMap::new();
            ctx.insert("alert_name".to_string(), "ServiceConnectionTimeout".to_string());
            ctx.insert("source_service".to_string(), "frontend".to_string());
            ctx.insert("target_service".to_string(), "inventory-api".to_string());
            ctx.insert("error_rate".to_string(), "35%".to_string());
            ctx.insert("timeout_duration".to_string(), "30s".to_string());
            
            ("Frontend service is experiencing connection timeouts to inventory API. Investigate network/service issues.", ctx)
        }
        _ => {
            eprintln!("Unknown scenario: {}. Available: pod-crash, high-cpu, memory-leak, network-issue", scenario);
            return Ok(());
        }
    };
    
    let llm_config = LLMConfig {
        provider: provider.to_string(),
        ..Default::default()
    };
    
    let agent_runtime = AgentRuntime::new(llm_config)?;
    let result = agent_runtime.investigate(&goal, context).await?;
    
    print_results(&result);
    
    Ok(())
}

async fn run_chatbot_mode(provider: &str, model: Option<String>) -> Result<()> {
    println!("=== Interactive Chatbot Mode ===");
    println!("Provider: {}", provider);
    if let Some(ref m) = model {
        println!("Model: {}", m);
    }
    println!("Type 'exit' to quit\n");
    
    // Check for API key if using cloud providers
    if provider == "anthropic" && env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Error: ANTHROPIC_API_KEY environment variable not set.");
        eprintln!("Please set it or use --provider mock for testing without an API key.");
        return Ok(());
    }
    
    if provider == "openai" && env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set.");
        eprintln!("Please set it or use --provider mock for testing without an API key.");
        return Ok(());
    }
    
    // Set up the LLM config
    let llm_config = LLMConfig {
        provider: provider.to_string(),
        model: model.unwrap_or_else(|| match provider {
            "anthropic" => "claude-3-sonnet-20240229".to_string(),
            "openai" => "gpt-4".to_string(),
            _ => "mock".to_string(),
        }),
        api_key: match provider {
            "anthropic" => env::var("ANTHROPIC_API_KEY").ok(),
            "openai" => env::var("OPENAI_API_KEY").ok(),
            _ => None,
        },
        ..Default::default()
    };
    
    let mut agent_runtime = AgentRuntime::new(llm_config)?;
    
    // Always add tools for chatbot
    println!("Initializing tools...");
    let prometheus_endpoint = env::var("PROMETHEUS_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:9090".to_string());
    
    // Try to add kubectl tool with automatic configuration
    match KubectlTool::infer().await {
        Ok(kubectl_tool) => {
            // Get cluster context information
            match kubectl_tool.get_cluster_context().await {
                Ok(context) => {
                    println!("✅ kubectl tool initialized with inferred configuration");
                    println!("\n📋 Cluster Context:");
                    for line in context.lines() {
                        println!("   {}", line);
                    }
                    println!();
                }
                Err(e) => {
                    println!("✅ kubectl tool initialized (context fetch failed: {})", e);
                }
            }
            agent_runtime.add_tool("kubectl".to_string(), kubectl_tool);
        }
        Err(e) => {
            println!("⚠️  kubectl tool not available: {}", e);
        }
    }
    
    agent_runtime.add_tool("promql".to_string(), PromQLTool::new(prometheus_endpoint));
    agent_runtime.add_tool("curl".to_string(), CurlTool::new());
    agent_runtime.add_tool("script".to_string(), ScriptTool::new());

    println!("Tools initialized: {:?}", agent_runtime.list_tools());
    
    let chatbot = agent_runtime.get_chatbot_agent();
    let mut history: Vec<Message> = Vec::new();
    let session_id = format!("cli-session-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs());
    
    println!("Chatbot ready! You can ask questions about your Kubernetes cluster.\n");
    
    loop {
        print!("> ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        if input == "exit" || input == "quit" {
            println!("Goodbye!");
            break;
        }
        
        // Create chat input
        let agent_input = AgentInput::ChatMessage {
            content: input.to_string(),
            history: history.clone(),
            session_id: Some(session_id.clone()),
            user_id: Some("cli-user".to_string()),
        };
        
        // Get response
        match agent_runtime.execute(&chatbot, agent_input).await? {
            AgentOutput::ChatResponse { message, suggested_actions, .. } => {
                println!("\nChatbot: {}\n", message);
                
                if let Some(actions) = suggested_actions {
                    println!("Suggested actions:");
                    for action in actions {
                        println!("  - {}", action);
                    }
                    println!();
                }
                
                // Add to history
                history.push(Message::user(input));
                history.push(Message::assistant(&message));
            }
            AgentOutput::Error { message, .. } => {
                eprintln!("Error: {}", message);
            }
            _ => {
                eprintln!("Unexpected response type from chatbot");
            }
        }
    }
    
    Ok(())
}

async fn run_investigator_mode_interactive(provider: &str, enable_approval: bool) -> Result<()> {
    println!("=== Interactive Investigator Mode ===");
    println!("Provider: {}", provider);
    println!("Approval simulation: {}", enable_approval);
    println!();
    
    // Interactive investigation form
    println!("🔍 Investigation Setup");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Get investigation goal
    print!("What do you want to investigate? (describe the issue): ");
    io::stdout().flush()?;
    let mut goal = String::new();
    io::stdin().read_line(&mut goal)?;
    let goal = goal.trim();
    
    let goal = if goal.is_empty() {
        println!("No goal provided, using default investigation");
        "General cluster health check".to_string()
    } else {
        goal.to_string()
    };
    
    // Get alert context
    println!("\n📊 Alert/Issue Context (optional)");
    println!("Enter relevant details about the alert or issue:");
    let mut alert_context = HashMap::new();
    
    // Common alert fields
    let fields = vec![
        ("alert_name", "Alert name (e.g., PodCrashLooping, HighCPUUsage)"),
        ("namespace", "Namespace affected"),
        ("service", "Service/application affected"),
        ("pod", "Specific pod name (if applicable)"),
        ("severity", "Severity level (critical, warning, info)"),
        ("duration", "How long has this been happening?"),
        ("symptoms", "What symptoms are you seeing?"),
        ("recent_changes", "Any recent changes or deployments?"),
    ];
    
    println!("You can provide any of these details (press Enter to skip):");
    for (key, description) in &fields {
        print!("  {}: ", description);
        io::stdout().flush()?;
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;
        let value = value.trim();
        
        if !value.is_empty() {
            alert_context.insert(key.to_string(), value.to_string());
        }
    }
    
    // Additional custom context
    println!("\n📝 Additional Context");
    println!("Add any other relevant key-value pairs (empty key to finish):");
    loop {
        print!("Key: ");
        io::stdout().flush()?;
        let mut key = String::new();
        io::stdin().read_line(&mut key)?;
        let key = key.trim();
        
        if key.is_empty() {
            break;
        }
        
        print!("Value: ");
        io::stdout().flush()?;
        let mut value = String::new();
        io::stdin().read_line(&mut value)?;
        let value = value.trim();
        
        alert_context.insert(key.to_string(), value.to_string());
    }
    
    // Display summary
    println!("\n📋 Investigation Summary");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Goal: {}", goal);
    if !alert_context.is_empty() {
        println!("Context:");
        for (key, value) in &alert_context {
            println!("  {}: {}", key, value);
        }
    }
    println!();
    
    // Confirm to proceed
    print!("Proceed with investigation? (y/N): ");
    io::stdout().flush()?;
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm)?;
    let confirm = confirm.trim().to_lowercase();
    
    if confirm != "y" && confirm != "yes" {
        println!("Investigation cancelled.");
        return Ok(());
    }
    
    // Check for API key if using cloud providers
    if provider == "anthropic" && env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Error: ANTHROPIC_API_KEY environment variable not set.");
        eprintln!("Please set it or use --provider mock for testing without an API key.");
        return Ok(());
    }
    
    if provider == "openai" && env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Error: OPENAI_API_KEY environment variable not set.");
        eprintln!("Please set it or use --provider mock for testing without an API key.");
        return Ok(());
    }
    
    // Get cluster context
    let cluster_context = get_cluster_context().await;
    if let Some(ref context) = cluster_context {
        println!("📋 Cluster Context:");
        for line in context.lines() {
            println!("   {}", line);
        }
        println!();
    }
    
    let llm_config = LLMConfig {
        provider: provider.to_string(),
        model: match provider {
            "anthropic" => "claude-3-sonnet-20240229".to_string(),
            "openai" => "gpt-4".to_string(),
            _ => "mock".to_string(),
        },
        api_key: match provider {
            "anthropic" => env::var("ANTHROPIC_API_KEY").ok(),
            "openai" => env::var("OPENAI_API_KEY").ok(),
            _ => None,
        },
        ..Default::default()
    };
    
    let mut agent_runtime = AgentRuntime::new(llm_config)?;
    
    // Add tools
    println!("Initializing tools...");
    let prometheus_endpoint = env::var("PROMETHEUS_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:9090".to_string());
    
    // Try to add kubectl tool with automatic configuration
    match KubectlTool::infer().await {
        Ok(kubectl_tool) => {
            agent_runtime.add_tool("kubectl".to_string(), kubectl_tool);
            println!("✅ kubectl tool initialized");
        }
        Err(e) => {
            println!("⚠️  kubectl tool not available: {}", e);
        }
    }
    
    agent_runtime.add_tool("promql".to_string(), PromQLTool::new(prometheus_endpoint));
    agent_runtime.add_tool("curl".to_string(), CurlTool::new());
    agent_runtime.add_tool("script".to_string(), ScriptTool::new());
    
    let investigator = agent_runtime.get_investigator_agent();
    
    // Create investigation input with cluster context and alert context
    let workflow_id = "cli-investigation".to_string();
    let mut initial_data = serde_json::json!({
        "source": "cli",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    });
    
    if let Some(context) = cluster_context {
        initial_data["cluster_context"] = serde_json::Value::String(context);
    }
    
    let input = AgentInput::InvestigationGoal {
        goal: goal.clone(),
        initial_data,
        workflow_id: workflow_id.clone(),
        alert_context: if alert_context.is_empty() { None } else { Some(alert_context) },
    };
    
    println!("🔍 Starting investigation...\n");
    
    // Run investigation
    let output = agent_runtime.execute(&investigator, input).await?;
    
    match output {
        AgentOutput::FinalInvestigationResult(result) => {
            print_results(&result);
        }
        AgentOutput::PendingHumanApproval { 
            request_message, 
            options, 
            current_investigation_state,
            risk_level,
            ..
        } => {
            println!("=== Human Approval Required ===");
            println!("Risk Level: {:?}", risk_level);
            println!("\n{}\n", request_message);
            println!("Options: {}", options.join(", "));
            
            if enable_approval {
                print!("\nYour choice (or press Enter to deny): ");
                io::stdout().flush()?;
                
                let mut choice = String::new();
                io::stdin().read_line(&mut choice)?;
                let choice = choice.trim();
                
                let approved = choice.to_lowercase() == "approve";
                
                // Create approval response
                let approval_input = AgentInput::ResumeInvestigation {
                    original_goal: goal.clone(),
                    approval_response: punching_fist_operator::agent::behavior::HumanApprovalResponse {
                        approved,
                        feedback: if approved {
                            Some("Approved via CLI".to_string())
                        } else {
                            Some("Denied via CLI".to_string())
                        },
                        selected_option: Some(choice.to_string()),
                        approver: "cli-user".to_string(),
                        approval_time: Utc::now(),
                    },
                    saved_state: current_investigation_state,
                    workflow_id,
                };
                
                // Get final result after approval
                let final_output = agent_runtime.execute(&investigator, approval_input).await?;
                
                match final_output {
                    AgentOutput::FinalInvestigationResult(result) => {
                        println!("\n=== Final Result After {} ===", 
                            if approved { "Approval" } else { "Denial" });
                        print_results(&result);
                    }
                    _ => {
                        eprintln!("Unexpected output after approval");
                    }
                }
            } else {
                println!("\n(Approval simulation disabled - use --approval to enable)");
            }
        }
        AgentOutput::Error { message, .. } => {
            eprintln!("Investigation error: {}", message);
        }
        _ => {
            eprintln!("Unexpected output type from investigator");
        }
    }
    
    Ok(())
}

fn print_results(result: &punching_fist_operator::agent::result::AgentResult) {
    println!("=== Investigation Results ===");
    println!();
    println!("Summary: {}", result.summary);
    println!("Confidence: {:.0}%", result.confidence * 100.0);
    println!("Can Auto-Fix: {}", result.can_auto_fix);
    
    if let Some(root_cause) = &result.root_cause {
        println!("\nRoot Cause:\n  {}", root_cause);
    }
    
    if !result.findings.is_empty() {
        println!("\nFindings:");
        for (i, finding) in result.findings.iter().enumerate() {
            println!("  {}. [{:?}] {} - {}", 
                i + 1, 
                finding.severity, 
                finding.category,
                finding.description
            );
        }
    }
    
    if !result.recommendations.is_empty() {
        println!("\nRecommendations:");
        for rec in &result.recommendations {
            println!("  - [Priority {}] {}", rec.priority, rec.action);
            println!("    Risk: {:?}, Requires Approval: {}", rec.risk_level, rec.requires_approval);
        }
    }
    
    if let Some(fix_cmd) = &result.fix_command {
        println!("\nAuto-Fix Command:\n  {}", fix_cmd);
    }
    
    if !result.actions_taken.is_empty() {
        println!("\nActions Taken:");
        for action in &result.actions_taken {
            let status_icon = if action.success { "✓" } else { "✗" };
            println!("  {} {} ({}): {}", 
                status_icon,
                action.tool, 
                action.timestamp.format("%H:%M:%S"), 
                action.command
            );
            println!("    Output: {}", action.output_summary);
        }
    }
} 