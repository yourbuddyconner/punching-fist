# Local Kubernetes Deployment Guide

This guide explains how to deploy the punching-fist-operator to your local Kubernetes cluster using Rancher Desktop.

## Prerequisites

- [Rancher Desktop](https://rancherdesktop.io/) or another local Kubernetes cluster
- [Helm](https://helm.sh/docs/intro/install/) 3.x installed
- [Just](https://github.com/casey/just) command runner installed
- kubectl configured to access your local cluster

## Quick Start

### 1. Deploy without OpenHands API key (for testing)

```bash
# Build and deploy in one command
just deploy-local
```

### 2. Deploy with OpenHands API key (for full functionality)

```bash
# Deploy with your actual API key
just deploy-local-with-key "your-api-key-here"
```

## Step-by-Step Deployment

### 1. Build the Docker Image

```bash
# Build the Docker image locally
just docker-build-local
```

This creates a `punching-fist:latest` image in your local Docker registry.

### 2. Deploy to Kubernetes

```bash
# Deploy using Helm
just k8s-deploy

# Or with API key
just k8s-deploy-with-key "your-api-key-here"
```

This deploys the operator to the `punching-fist-system` namespace.

### 3. Check Deployment Status

```bash
# Check if pods are running
just k8s-status
```

### 4. Access the Operator

```bash
# Port forward to access locally
just k8s-port-forward
```

The operator will be available at `http://localhost:8080`.

### 5. View Logs

```bash
# Follow the operator logs
just k8s-logs
```

## Available Endpoints

Once deployed and port-forwarded, you can access:

- **Health Check**: `GET http://localhost:8080/health`
- **Metrics**: `GET http://localhost:8080/metrics`
- **Alert Handler**: `POST http://localhost:8080/alerts`
- **Prometheus Alerts**: `POST http://localhost:8080/alerts/prometheus`

## Configuration

The deployment uses `charts/punching-fist/values-local.yaml` for local development settings:

- Uses local Docker images (`pullPolicy: Never`)
- Disables Prometheus ServiceMonitor by default
- Sets appropriate RBAC permissions
- Configures resource limits suitable for local development

## Cleanup

```bash
# Remove the deployment completely
just k8s-uninstall
```

## Troubleshooting

### Pod Fails to Start

Check the logs:
```bash
just k8s-logs
```

Common issues:
- Missing OpenHands API key (deploy with `deploy-local-with-key`)
- Image pull issues (ensure `docker-build-local` completed successfully)
- Insufficient permissions (check RBAC configuration)

### Health Check Fails

The operator exposes a `/health` endpoint that returns "OK". If health checks fail:
1. Check if the container is running: `just k8s-status`
2. Check logs for startup errors: `just k8s-logs`
3. Verify port 8080 is accessible in the container

### Can't Access via Port Forward

If `just k8s-port-forward` doesn't work:
```bash
# Check if the service exists
kubectl get svc -n punching-fist-system

# Manual port forward
kubectl port-forward deployment/punching-fist 8080:8080 -n punching-fist-system
```

## Advanced Configuration

For production or custom deployments, modify `charts/punching-fist/values.yaml` or create your own values file:

```bash
helm upgrade --install punching-fist charts/punching-fist \
    --values your-custom-values.yaml \
    --namespace punching-fist-system
``` 