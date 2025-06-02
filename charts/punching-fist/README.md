# End-to-End Test Setup for Punching Fist Operator

This directory contains the complete end-to-end test infrastructure for the Punching Fist operator. The test setup creates a full pipeline from problematic pods → Prometheus alerts → AlertManager → Punching Fist → AI investigation.

## Components

### 1. Test Workloads (`test-pods.yaml`)
Deploys various problematic pods to trigger alerts:
- **healthy-app**: A normal working pod for comparison
- **memory-hog**: Uses stress to consume excessive memory
- **crashloop-app**: Exits after 5 minutes to create restart loops
- **cpu-intensive**: Uses stress to consume excessive CPU

### 2. Prometheus Rules (`prometheus-rules.yaml`)
Creates PrometheusRule resources that monitor the test workloads:
- **TestPodCrashLooping**: Fires when pods restart frequently
- **TestPodHighMemoryUsage**: Fires when memory usage > 80%
- **TestPodHighCPUUsage**: Fires when CPU usage > 80%
- **TestPodNotReady**: Fires when pods are not ready for 5+ minutes

### 3. Test Pipeline (`test-pipeline.yaml`)
Deploys the Punching Fist CRDs for the test flow:
- **Source**: Webhook endpoint listening at `/webhook/test-alerts`
- **Workflow**: AI agent that investigates alerts using kubectl and PromQL
- **Sink**: Stdout output (and optional Slack notifications)

## Alert Flow

```
┌─────────────────┐
│  Test Workload  │ (crashloop-app, memory-hog, etc.)
└────────┬────────┘
         │ Metrics
         ▼
┌─────────────────┐
│   Prometheus    │ Scrapes metrics, evaluates rules
└────────┬────────┘
         │ Alert fires
         ▼
┌─────────────────┐
│  AlertManager   │ Routes alert to webhook
└────────┬────────┘
         │ POST /webhook/test-alerts
         ▼
┌─────────────────┐
│ Punching Fist   │ 
│    Source       │ Receives alert, triggers workflow
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Punching Fist   │ 
│   Workflow      │ AI agent investigates using kubectl/PromQL
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Punching Fist   │ 
│     Sink        │ Outputs results to stdout/Slack
└─────────────────┘
```

## Integration with AlertManager

### Overview

The Punching Fist operator integrates with AlertManager to automatically receive and process alerts. When deployed with the Prometheus stack, AlertManager is pre-configured to route alerts to the operator's webhook endpoints.

### Configuration

In the local test environment (`values-local.yaml`), AlertManager is configured with:

1. **Test-specific webhook** (`/webhook/test-alerts`): Handles test alerts from the test workloads
2. **Generic webhook** (`/webhook/alertmanager`): Handles all other alerts

AlertManager routes alerts based on:
- Alert namespace (test-workloads or release namespace)
- Alert name (TestPodCrashLooping, TestPodHighMemoryUsage, etc.)
- Catch-all rule for any other alerts

### Alert Processing Flow

1. **Alert Fired**: Prometheus detects a condition and fires an alert
2. **AlertManager Routing**: AlertManager groups and routes the alert to Punching Fist webhook
3. **Source Processing**: The webhook Source receives the alert and triggers the configured workflow
4. **Workflow Execution**: The workflow runs with agent steps to investigate the alert
5. **Results Output**: Investigation results are sent to configured sinks (stdout, Slack, etc.)

### Custom Alert Workflows

You can create custom workflows for specific alerts by:

1. Creating a new Source with specific filters:
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Source
metadata:
  name: my-critical-alerts
spec:
  type: webhook
  config:
    path: "/webhook/critical"
    filters:
      severity: ["critical"]
  triggerWorkflow: "critical-alert-handler"
```

2. Configuring AlertManager to route to your webhook:
```yaml
alertmanager:
  config:
    routes:
    - match:
        severity: critical
      receiver: my-critical-webhook
    receivers:
    - name: my-critical-webhook
      webhook_configs:
      - url: 'http://punching-fist:8080/webhook/critical'
```

### Monitoring Alert Processing

View alert processing in the web dashboard:
- **Alerts tab**: Shows all received alerts with status
- **Workflows tab**: Shows triggered workflows and their progress
- **Source Events tab**: Shows raw webhook events received

### Troubleshooting

1. **Alerts not received**: Check AlertManager logs and webhook connectivity
2. **Workflows not triggered**: Verify Source filters match alert labels
3. **Investigation failures**: Check workflow logs and agent permissions

## Configuration

The test setup is controlled via Helm values:

```yaml
testResources:
  enabled: true  # Enable test workloads
  namespace: test-workloads
  
  prometheusRules:
    enabled: true  # Create PrometheusRules
  
  pipeline:
    enabled: true  # Deploy Source/Workflow/Sink
    maxIterations: 10
    timeoutMinutes: 5
    
    # Optional Slack integration
    enableSlack: false
    slackChannel: "#test-alerts"
    slackTokenSecret: "slack-bot-token"
```

## Usage

### Deploy the E2E Test Environment

```bash
# Deploy everything with test resources enabled
just e2e-deploy

# Or manually with Helm
helm install punching-fist ./charts/punching-fist \
  --values ./charts/punching-fist/values-local.yaml \
  --set agent.anthropicApiKey=$ANTHROPIC_API_KEY \
  --namespace punching-fist \
  --create-namespace
```

### Monitor the Test Flow

```bash
# Check all components status
just e2e-status

# View active Prometheus alerts
just e2e-alerts

# Watch operator logs to see investigations
just e2e-logs

# View workflow executions
just e2e-workflows
```

### Trigger Test Scenarios

```bash
# Manually send a test alert
just e2e-trigger

# Force crashloop pod to restart
just e2e-restart-crashloop

# The test pods will naturally trigger alerts:
# - crashloop-app: Restarts every 5 minutes
# - memory-hog: Constantly uses high memory
# - cpu-intensive: Constantly uses high CPU
```

### Access UIs

```bash
# Prometheus UI
kubectl port-forward -n punching-fist svc/punching-fist-prometheus-s-prometheus 9090:9090
# Visit http://localhost:9090

# AlertManager UI
kubectl port-forward -n punching-fist svc/punching-fist-prometheus-s-alertmanager 9093:9093
# Visit http://localhost:9093

# Punching Fist API
kubectl port-forward -n punching-fist svc/punching-fist 8080:8080
# Visit http://localhost:8080
```

## Cleanup

```bash
# Clean up test resources only
just e2e-clean-tests

# Full cleanup including operator
just e2e-cleanup
```

## Troubleshooting

### No alerts firing
1. Check if test pods are running: `kubectl get pods -n test-workloads`
2. Verify PrometheusRules are loaded: `kubectl get prometheusrules -n test-workloads`
3. Check Prometheus targets: Visit Prometheus UI → Status → Targets
4. Query metrics directly: `container_memory_working_set_bytes{namespace="test-workloads"}`

### Alerts not reaching Punching Fist
1. Check AlertManager config: `kubectl get secret -n punching-fist alertmanager-punching-fist-prometheus-s-alertmanager -o yaml`
2. View AlertManager logs: `kubectl logs -n punching-fist alertmanager-punching-fist-prometheus-s-alertmanager-0`
3. Verify webhook endpoint: `curl http://localhost:8080/webhook/test-alerts` (with port-forward)

### Workflow not executing
1. Check Source CRD: `kubectl describe source punching-fist-test-webhook -n punching-fist`
2. View operator logs: `kubectl logs -n punching-fist statefulset/punching-fist`
3. Check WorkflowExecutions: `kubectl get workflowexecutions -n punching-fist` 