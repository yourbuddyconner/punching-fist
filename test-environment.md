# Test Environment for Punching Fist Operator

This directory contains a complete test environment for the Punching Fist operator, allowing you to test the operator on any Kubernetes cluster using your current kubeconfig context.

## Features

- **Any Kubernetes cluster**: Works with your current kubeconfig context (k3s, minikube, kind, EKS, GKE, etc.)
- **Integrated Prometheus**: Deployed as a subchart with the operator for metrics collection
- **Test workloads**: Sample problematic pods (crashloop, memory hog, CPU intensive) for testing operator functionality
- **Automated deployment**: Simple commands to build, deploy, and test the operator
- **All-in-one Helm chart**: Everything deploys together with configurable feature flags

## Prerequisites

- kubectl configured with access to a Kubernetes cluster
- Docker (for building operator images)
- Helm 3
- just (command runner)
- Rust toolchain (for building the operator)

## Quick Start

1. **Check your current Kubernetes context:**
   ```bash
   just test-context
   ```

2. **Deploy the complete test environment:**
   ```bash
   just test-deploy
   ```
   This will:
   - Build the operator Docker image
   - Deploy the operator with Helm
   - Deploy Prometheus (enabled by default in local values)
   - Deploy test workloads (enabled by default in local values)

3. **Access the services:**
   - **Operator UI**: Run `just test-port-forward-operator` then visit http://localhost:8080
   - **Prometheus UI**: Run `just test-port-forward-prometheus` then visit http://localhost:9090
   - If your cluster supports NodePort services, Prometheus may also be available at http://localhost:30090

4. **View logs:**
   ```bash
   just test-logs
   ```

## Available Commands

### Test Environment Management
- `just test-context` - Show current Kubernetes context and cluster info
- `just test-deploy` - Full deployment (build and deploy everything)
- `just test-cleanup` - Clean up all test resources

### Operator Deployment
- `just test-build-operator` - Build operator Docker image
- `just test-helm-deps` - Update Helm chart dependencies
- `just test-deploy-operator` - Build and deploy operator to current context
- `just test-run-example` - Run test_rig_tools example
- `just test-port-forward-operator` - Port forward to operator service
- `just test-port-forward-prometheus` - Port forward to Prometheus service

### Monitoring
- `just test-logs` - View operator logs
- `just test-workloads-status` - Check status of test workloads

## Helm Chart Features

The punching-fist Helm chart includes:

1. **Core Operator**: The main punching-fist operator deployment
2. **Prometheus Stack** (optional): Full Prometheus deployment for metrics
3. **Test Resources** (optional): Sample problematic workloads for testing

### Configuration Options

Key values in `charts/punching-fist/values-local.yaml`:

```yaml
# Enable test workloads
testResources:
  enabled: true
  namespace: test-workloads

# Enable Prometheus stack
prometheus-stack:
  enabled: true
  prometheus:
    prometheusSpec:
      service:
        type: NodePort
        nodePort: 30090
```

## Test Workloads

When `testResources.enabled` is set to `true` (default for local development), the following test resources are deployed:

- **healthy-app**: A normal functioning nginx pod
- **memory-hog**: Pod consuming excessive memory
- **crashloop-app**: Pod that crashes every 10 seconds
- **cpu-intensive**: Pod with high CPU usage

These pods are deployed in the `test-workloads` namespace and can be used to test the operator's investigation and remediation capabilities.

## Docker Image Management

Since we're no longer using a specific k3s docker-compose setup, you'll need to ensure your Docker images are available to your Kubernetes cluster:

### For Local Clusters (minikube, kind, etc.)
- **minikube**: Use `eval $(minikube docker-env)` before building
- **kind**: Use `kind load docker-image punching-fist:latest` after building
- **k3d**: Use `k3d image import punching-fist:latest` after building

### For Remote Clusters
You'll need to push to a container registry:
```bash
docker tag punching-fist:latest your-registry/punching-fist:latest
docker push your-registry/punching-fist:latest
# Then update the image in values-local.yaml
```

## Architecture

```
┌─────────────────────┐
│   Your K8s Cluster  │
│                     │
│  ┌───────────────┐  │
│  │ punching-fist │  │
│  │ namespace     │  │
│  │               │  │
│  │ ┌───────────┐ │  │
│  │ │punching-  │ │  │
│  │ │fist       │ │  │
│  │ │operator   │ │  │
│  │ └───────────┘ │  │
│  │               │  │
│  │ ┌───────────┐ │  │
│  │ │prometheus │ │  │
│  │ │stack      │ │  │
│  │ └───────────┘ │  │
│  └───────────────┘  │
│                     │
│  ┌───────────────┐  │
│  │test-workloads │  │
│  │namespace      │  │
│  │               │  │
│  │ ┌───────────┐ │  │
│  │ │test pods  │ │  │
│  │ └───────────┘ │  │
│  └───────────────┘  │
└─────────────────────┘
```

## Troubleshooting

### Can't connect to cluster
- Check your current context: `kubectl config current-context`
- Verify cluster access: `kubectl cluster-info`
- Ensure you have proper permissions: `kubectl auth can-i create deployments`

### Operator image not found
- Build the image first: `just test-build-operator`
- For local clusters, ensure the image is available (see Docker Image Management section)
- For remote clusters, push to a registry and update values

### Test pods not appearing
- Verify testResources.enabled is true in values
- Check the namespace: `kubectl get pods -n test-workloads`
- Check for any errors: `kubectl describe pods -n test-workloads`

### Prometheus not accessible
- Check if Prometheus pods are running: `kubectl get pods -n punching-fist -l app.kubernetes.io/name=prometheus`
- Try port-forwarding: `just test-port-forward-prometheus`
- For NodePort access, ensure your cluster supports NodePort services

### Helm dependency errors
- Run `just test-helm-deps` to update chart dependencies
- Ensure you have internet connectivity to download Prometheus chart

### Cleanup issues
- If `just test-cleanup` fails, manually delete resources:
  ```bash
  kubectl delete namespace punching-fist --force --grace-period=0
  kubectl delete namespace test-workloads --force --grace-period=0
  ``` 