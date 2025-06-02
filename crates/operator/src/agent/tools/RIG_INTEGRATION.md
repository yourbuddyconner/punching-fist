# Rig Integration for Tools

This directory contains tool implementations that integrate with the [Rig](https://github.com/0xPlaygrounds/rig) agent framework.

## Tool Architecture

Each tool implements Rig's `Tool` trait, providing:
- Type-safe parameter handling
- Automatic OpenAPI schema generation for LLMs
- Consistent error handling
- Async execution

## Available Tools

### KubectlTool
Provides safe kubectl command execution for Kubernetes operations.

```rust
// Method 1: Automatic configuration detection
let kubectl = KubectlTool::infer().await?;
// This will automatically use:
// 1. Kubeconfig from KUBECONFIG env var or ~/.kube/config
// 2. In-cluster service account if kubeconfig is not available

// Method 2: Explicit client
let k8s_client = Client::try_default().await?;
let kubectl = KubectlTool::new(k8s_client);
```

**Supported Resources:**
- `pods` / `pod` - Get individual pods or list all pods in a namespace
- `namespaces` / `namespace` / `ns` - Get individual namespaces or list all namespaces

**Supported Commands:**
- `kubectl get pods [-n namespace]` - List pods
- `kubectl get pod <name> [-n namespace]` - Get specific pod details
- `kubectl get namespaces` - List all namespaces
- `kubectl get namespace <name>` - Get specific namespace details

Additional commands like `describe`, `logs`, `top`, and `events` are recognized but not yet fully implemented.

## Overview

All tools in this module implement Rig's `Tool` trait directly, enabling seamless integration with Rig's agent system and LLM providers.

## Implementation Details

Each tool (KubectlTool, PromQLTool, CurlTool, ScriptTool) implements the Rig Tool trait with:

- `const NAME`: Tool identifier
- `type Error = ToolError`: Common error type for all tools
- `type Args = ToolArgs`: Simple command string argument
- `type Output = ToolResult`: Structured result with success/error info
- `definition()`: Returns OpenAPI-style parameter schema for the LLM
- `call()`: Executes the tool with validation and error handling

## Usage with Rig Agents

```rust
use rig::providers::openai;
use rig::tool::ToolSet;
use punching_fist_operator::agent::tools::{kubectl::KubectlTool, promql::PromQLTool};

// Create tools
let kubectl = KubectlTool::new(k8s_client);
let promql = PromQLTool::new("http://prometheus:9090".to_string());

// Add to ToolSet
let toolset = ToolSet::builder()
    .tool(kubectl)
    .tool(promql)
    .build();

// Create agent with tools
let agent = openai::Client::from_env()
    .agent("gpt-4")
    .dynamic_tools(toolset)
    .build();

// The agent can now use kubectl and promql tools during conversations
```

## Tool Definitions

Each tool provides a JSON schema definition that tells the LLM:
- What parameters the tool accepts
- What each parameter means
- Which parameters are required

Example kubectl definition:
```json
{
  "name": "kubectl",
  "description": "Execute kubectl commands for Kubernetes cluster inspection",
  "parameters": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string", 
        "description": "The kubectl command to execute (e.g., 'kubectl get pods -n default')"
      }
    },
    "required": ["command"]
  }
}
```

## Safety Features

- All commands are validated before execution
- Dangerous commands (delete, patch, etc.) are detected
- Namespace restrictions can be applied
- Command sanitization prevents injection attacks

## Architecture Benefits

By using Rig's Tool trait directly:
- ✅ Single tool implementation for both internal use and LLM agents
- ✅ Type-safe integration with Rig's agent system
- ✅ Automatic tool discovery and registration
- ✅ Consistent error handling across all tools
- ✅ Built-in support for tool documentation and schemas

## Testing

Run the example to see the integration in action:
```bash
cargo run --example test_rig_tools
```

## Known Limitations

Some tools may have async/await compatibility issues with certain futures that don't implement `Sync`. This is being addressed in future updates. 