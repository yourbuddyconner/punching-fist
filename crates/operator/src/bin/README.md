# Agent Test CLI

A command-line tool for testing agent functionality in isolation.

## Building

```bash
cargo build --bin test-agent
```

## Configuration

### Environment Variables

The CLI supports loading environment variables from a `.env` file. This is useful for storing API keys and other configuration without having to export them in your shell.

1. Copy the example environment file:
   ```bash
   cp crates/operator/src/bin/env.example .env
   ```

2. Edit `.env` and add your API keys:
   ```env
   ANTHROPIC_API_KEY=your-actual-key-here
   OPENAI_API_KEY=your-actual-key-here
   ```

The `.env` file is automatically loaded when you run the CLI. If no `.env` file is found, the CLI will use system environment variables.

### Supported Environment Variables

- `ANTHROPIC_API_KEY`: API key for Anthropic/Claude
- `OPENAI_API_KEY`: API key for OpenAI
- `PROMETHEUS_ENDPOINT`: Prometheus server URL (default: http://localhost:9090)
- `LOG_LEVEL`: Default log level (debug, info, warn, error)

## Usage

The test-agent CLI provides several ways to test the agent functionality:

### 1. Mock Provider (No API Key Required)

Test with the mock provider for quick testing without API keys:

```bash
# Basic test
cargo run --bin test-agent -- mock

# Custom goal and alert
cargo run --bin test-agent -- mock --goal "Investigate high memory usage" --alert "HighMemory"

# With debug logging
cargo run --bin test-agent -- --log-level debug mock
```

### 2. Anthropic/Claude Provider

Test with real Anthropic API (requires ANTHROPIC_API_KEY):

```bash
# Basic test (uses env var ANTHROPIC_API_KEY)
cargo run --bin test-agent -- anthropic --goal "Investigate pod crash"

# With specific model
cargo run --bin test-agent -- anthropic --model claude-3-5-sonnet --goal "Debug network issue"

# With tools enabled
cargo run --bin test-agent -- anthropic --tools --goal "Analyze service performance"

# With custom API key
cargo run --bin test-agent -- anthropic --api-key YOUR_KEY --goal "Check deployment status"
```

### 3. OpenAI Provider

Test with OpenAI API (requires OPENAI_API_KEY):

```bash
# Basic test
cargo run --bin test-agent -- openai --goal "Investigate service outage"

# With GPT-4
cargo run --bin test-agent -- openai --model gpt-4 --goal "Analyze error logs"

# With tools
cargo run --bin test-agent -- openai --tools --goal "Debug database connection"
```

### 4. Interactive Mode

Interactive prompt for all inputs:

```bash
cargo run --bin test-agent -- interactive
```

This will prompt you for:
- Provider selection
- Model name (if applicable)
- Investigation goal
- Context key-value pairs

### 5. Pre-defined Scenarios

Test with realistic scenarios:

```bash
# Pod crash scenario
cargo run --bin test-agent -- scenario --name pod-crash

# High CPU scenario with Anthropic
cargo run --bin test-agent -- scenario --name high-cpu --provider anthropic

# Memory leak scenario
cargo run --bin test-agent -- scenario --name memory-leak

# Network issue scenario
cargo run --bin test-agent -- scenario --name network-issue
```

Available scenarios:
- `pod-crash`: Pod crash looping with exit code 137
- `high-cpu`: Service experiencing 98% CPU usage
- `memory-leak`: Service showing memory growth patterns
- `network-issue`: Service connection timeout errors

## Output

The tool displays:
- Investigation summary
- Confidence score
- Root cause analysis
- Findings with severity levels
- Prioritized recommendations
- Auto-fix capability and commands
- Actions taken during investigation
- Timestamp information

## Examples

```bash
# Quick test with mock provider
cargo run --bin test-agent -- mock

# Test a specific alert type
cargo run --bin test-agent -- mock --alert PodCrashLooping --goal "Pod keeps restarting"

# Run a realistic scenario
cargo run --bin test-agent -- scenario --name pod-crash

# Test with real LLM (requires API key)
export ANTHROPIC_API_KEY=your-key-here
cargo run --bin test-agent -- anthropic --goal "Service returns 500 errors"

# Interactive exploration
cargo run --bin test-agent -- interactive
``` 