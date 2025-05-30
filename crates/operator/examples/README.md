# Agent Testing Examples

This directory contains examples for testing the Punching Fist Operator's agent functionality.

## Available Examples

### 1. Basic Agent Test (`test_agent.rs`)
Tests the agent runtime directly with both mock and real providers.

```bash
# Test with mock provider (no API key needed)
cargo run --example test_agent

# Test with real OpenAI (requires API key)
OPENAI_API_KEY=your-key cargo run --example test_agent

# Test with real Anthropic (requires API key)
ANTHROPIC_API_KEY=your-key cargo run --example test_agent
```

### 2. Workflow Integration Test (`test_workflow_agent.rs`)
Tests the agent running within a workflow context.

```bash
# Requires Kubernetes context
cargo run --example test_workflow_agent
```

### 3. OpenAI Integration Test (`test_openai.rs`)
Focused test for real OpenAI integration.

```bash
OPENAI_API_KEY=your-key cargo run --example test_openai
```

### 4. Anthropic Claude Integration Test (`test_anthropic.rs`)
Test using Anthropic's Claude models.

```bash
# Test with mock provider (no API key needed)
cargo run --example test_anthropic

# Test with real Anthropic API
ANTHROPIC_API_KEY=your-key cargo run --example test_anthropic
```

## Setting Up

### Mock Provider (No API Key Required)
The mock provider simulates LLM responses for common alert types:
- `PodCrashLooping`: Memory limit issues
- `HighCPUUsage`: CPU scaling recommendations
- Generic alerts: Manual investigation required

### OpenAI Provider
1. Get an API key from [OpenAI](https://platform.openai.com/api-keys)
2. Set the environment variable: `export OPENAI_API_KEY=your-key`
3. Run any of the examples

### Anthropic Provider
1. Get an API key from [Anthropic](https://www.anthropic.com/)
2. Set the environment variable: `export ANTHROPIC_API_KEY=your-key`
3. Run the examples with Anthropic configuration

### Configuration Options
You can configure the LLM provider in your code:

```rust
let llm_config = LLMConfig {
    provider: "anthropic".to_string(),  // or "openai", "mock"
    model: "claude-3-5-sonnet".to_string(),  // or "gpt-4", etc.
    temperature: Some(0.7),
    max_tokens: Some(4096),
    ..Default::default()
};
```

### Supported Models

#### Anthropic Models
- `claude-3-5-sonnet` - Most capable, best for complex tasks
- `claude-3-opus` - Most powerful but slower
- `claude-3-sonnet` - Balanced performance
- `claude-3-haiku` - Fastest and most cost-effective

#### OpenAI Models
- `gpt-4` - Most capable
- `gpt-3.5-turbo` - Faster and more cost-effective

### Supported Alert Types
The agent can investigate various Kubernetes alerts:
- Pod crashes and restarts
- High CPU/memory usage
- Service unavailability
- Network connectivity issues
- Custom alerts

## Example Output

```
=== Testing with Anthropic Claude ===

Using real Anthropic API (Claude 3.5 Sonnet)

Investigation Summary: Pod crash investigation complete. Root cause: OutOfMemoryError due to insufficient memory limit.
Confidence: 0.85
Root Cause: The pod is crashing due to OutOfMemoryError...

Recommendations:
  - Increase memory limit to 1GB: Application requires more memory than currently allocated

Auto-fix available: kubectl patch deployment...
```

## Next Steps
- Implement sink handlers to send investigation results to Slack, PagerDuty, etc.
- Add more tool types (curl for HTTP checks, custom scripts)
- Enhance the LLM prompt parsing to extract actual tool calls
- Add support for more LLM providers (local models via Ollama) 