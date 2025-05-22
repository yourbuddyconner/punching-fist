# 👊🤖

A Kubernetes operator designed to run in your cluster and perform one-off or automated maintenance tasks. Built with Rust and Axum, Punching Fist provides a robust solution for cluster maintenance and automation.

## Overview

Punching Fist Operator (👊🤖) is a Kubernetes operator that leverages AI-powered automation to handle cluster maintenance tasks. It integrates with your existing monitoring stack and can respond to alerts or perform scheduled maintenance operations.

### Key Features

- 🤖 AI-powered maintenance using OpenHands in headless mode
- 🔄 Real-time alert response via webhook integration
- 📊 Prometheus metrics integration
- 🔧 Kubernetes-native operations
- 🛡️ Secure service account-based authentication
- 🚀 High-performance Rust implementation

## Architecture

The operator consists of several key components:

1. **Webhook Server**: Built with Axum, handles HTTP webhook requests from alerting systems
2. **OpenHands Integration**: Processes maintenance tasks using AI
3. **Kubernetes Client**: Manages cluster operations
4. **Prometheus Integration**: Collects and analyzes metrics

## Prerequisites

- Kubernetes cluster (v1.20+)
- Prometheus AlertManager
- OpenHands API access
- kubectl installed in the operator container

## Installation

```bash
# Add the Helm repository
helm repo add punching-fist https://your-helm-repo-url

# Install the operator
helm install punching-fist punching-fist/punching-fist \
  --namespace punching-fist \
  --create-namespace \
  --set openhands.apiKey=your-api-key
```

## Configuration

The operator can be configured through a ConfigMap or environment variables:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: punching-fist-config
data:
  openhands.apiKey: "your-api-key"
  server.port: "8080"
  prometheus.enabled: "true"
```

## Usage

### Alert Integration

Configure Prometheus AlertManager to send alerts to the operator:

```yaml
receivers:
- name: 'punching-fist'
  webhook_configs:
  - url: 'http://punching-fist:8080/webhook/alerts'
```

### Custom Maintenance Tasks

Create a maintenance task:

```yaml
apiVersion: maintenance.punchingfist.io/v1
kind: MaintenanceTask
metadata:
  name: cleanup-old-pods
spec:
  schedule: "0 0 * * *"
  action: "cleanup"
  parameters:
    age: "7d"
```

## Development

### Building from Source

```bash
# Clone the repository
git clone https://github.com/your-org/punching-fist-operator

# Build the operator
cargo build --release

# Build the container
docker build -t punching-fist:latest .
```

### Running Tests

```bash
cargo test
```

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For support, please open an issue in the GitHub repository or contact the maintainers. 