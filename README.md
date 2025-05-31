# Punching Fist Operator

A Kubernetes operator that provides intelligent incident response using LLM-powered investigation and automated remediation.

## Overview

Punching Fist is a Kubernetes operator designed to:
- 📊 Listen for alerts from AlertManager and other monitoring systems
- 🔍 Perform intelligent investigation using LLM agents
- 🤖 Execute safe remediation actions based on investigation findings
- 📝 Generate detailed reports of findings and actions taken

## Architecture

The operator implements a Source → Workflow → Sink pipeline:

1. **Sources**: Receive alerts from various monitoring systems (AlertManager, Prometheus, custom webhooks)
2. **Workflows**: Define investigation and remediation steps with LLM-powered decision making
3. **Sinks**: Send results to various destinations (Slack, PagerDuty, custom webhooks)

## Quick Start

### Prerequisites

- Kubernetes cluster
- kubectl configured
- Helm 3
- LLM API access (Anthropic Claude or OpenAI)

### Installation

```bash
# Deploy with Helm
helm install punching-fist ./charts/punching-fist \
  --namespace punching-fist \
  --create-namespace

# Or deploy with a specific API key
helm install punching-fist ./charts/punching-fist \
  --namespace punching-fist \
  --create-namespace \
  --set agent.anthropicApiKey=your-api-key
```

### Local Development

```bash
# Install dependencies
just install

# Run locally
just run

# Run tests
just test
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_TYPE` | Database type (sqlite or postgres) | `sqlite` |
| `DATABASE_URL` | PostgreSQL connection string | - |
| `SQLITE_PATH` | Path to SQLite database file | `data/punching-fist.db` |
| `SERVER_ADDR` | Server listen address | `0.0.0.0:8080` |
| `ANTHROPIC_API_KEY` | API key for Anthropic Claude | - |
| `OPENAI_API_KEY` | API key for OpenAI | - |
| `LLM_PROVIDER` | LLM provider (anthropic, openai, mock) | `anthropic` |
| `LLM_MODEL` | Default LLM model | `claude-3-5-sonnet` |
| `KUBE_NAMESPACE` | Kubernetes namespace | `default` |
| `EXECUTION_MODE` | Execution mode (local or kubernetes) | `local` |

**Note:** Either `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` must be set for LLM functionality. If neither is set, the operator will use a mock provider for testing.

### Example Workflow

```yaml
apiVersion: punching-fist.io/v1alpha1
kind: Workflow
metadata:
  name: pod-crash-investigation
spec:
  trigger:
    source: alertmanager
    filters:
      alertname: PodCrashLooping
  steps:
    - name: investigate
      type: agent
      config:
        prompt: |
          Investigate why pod {{ .alert.labels.pod }} is crash looping.
          Use kubectl to check logs and describe the pod.
        tools:
          - kubectl
          - promql
    - name: notify
      type: sink
      config:
        sink: slack
        message: |
          Pod {{ .alert.labels.pod }} investigation complete:
          {{ .steps.investigate.output }}
```

## Testing

### Running Tests

```bash
# Run all tests
just test

# Run specific test
just test-one test_name
```

### Manual Testing

```bash
# Send a test alert
just test-alert

# Check operator logs
kubectl logs -n punching-fist deployment/punching-fist
```

## Development

### Project Structure

```
.
├── crates/
│   └── operator/          # Main operator code
│       ├── src/
│       │   ├── agent/     # LLM agent runtime
│       │   ├── controllers/ # Kubernetes controllers
│       │   ├── sources/   # Alert sources
│       │   ├── workflow/  # Workflow engine
│       │   └── sinks/     # Output destinations
│       └── tests/         # Integration tests
├── charts/                # Helm charts
└── examples/              # Example configurations
```

### Building

```bash
# Build locally
just build

# Build Docker image
just docker-build

# Push to registry
just docker-push
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests
5. Submit a pull request

## License

Apache License 2.0 