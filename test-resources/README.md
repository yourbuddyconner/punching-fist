# Punching Fist Operator - End-to-End Testing Guide

## 🎯 Milestone: MVP Pipeline Validation

This guide walks through testing the complete Phase 1 pipeline:
**AlertManager → Source → Workflow → Agent → Sink (stdout)**

## Prerequisites

1. **Kubernetes cluster** (Rancher Desktop, Docker Desktop, or minikube)
2. **Anthropic API key** (set in `.env` file as `ANTHROPIC_API_KEY`)
3. **Helm 3.x** installed
4. **kubectl** configured for your cluster

## Step 1: Deploy the Operator

### Option A: Using Justfile (Recommended)
```bash
# Deploy everything including test workloads
just test-deploy
```

### Option B: Manual Deployment
```bash
# Deploy operator with Helm (CRDs are included automatically)
helm install punching-fist charts/punching-fist \
  --values charts/punching-fist/values-local.yaml \
  --set agent.anthropicApiKey=$ANTHROPIC_API_KEY \
  --namespace punching-fist \
  --create-namespace

# Wait for operator to be ready
kubectl wait --for=condition=ready pod -l app=punching-fist -n punching-fist --timeout=60s
```

## Step 2: Apply Test Resources

The test resources demonstrate the complete pipeline with stdout output:

```bash
# Apply the test Source, Workflow, and Sink
kubectl apply -f examples/test-stdout-pipeline.yaml
```

This creates:
- **Source**: Webhook endpoint at `/webhook/test`
- **Workflow**: Agent-based investigation using Anthropic Claude
- **Sink**: Stdout output for easy viewing

## Step 3: Verify Deployment

```bash
# Check operator is running
kubectl get pods -n punching-fist

# Check test workloads are running
kubectl get pods -n test-workloads

# Check custom resources are created
kubectl get sources,workflows,sinks -n punching-fist
```

Expected output:
```
NAME                           AGE
source.punchingfist.io/test-webhook-source   1m

NAME                             AGE
workflow.punchingfist.io/test-investigation  1m

NAME                        AGE
sink.punchingfist.io/stdout-debug  1m
```

## Step 4: Port Forward to Operator

```bash
# In a separate terminal
just test-port-forward-operator

# Or manually
kubectl port-forward -n punching-fist svc/punching-fist 8080:8080
```

## Step 5: Send Test Alerts

### Option A: Using Test Script (Interactive)
```bash
./test-resources/send-test-alert.sh
```

Choose from the menu:
1. PodCrashLooping - Tests investigation of crashing pod
2. HighCPUUsage - Tests CPU resource investigation
3. HighMemoryUsage - Tests memory resource investigation

### Option B: Manual curl
```bash
# Send a PodCrashLooping alert
curl -X POST http://localhost:8080/webhook/test \
  -H "Content-Type: application/json" \
  -d '{
    "version": "4",
    "groupKey": "{}:{alertname=\"PodCrashLooping\"}",
    "status": "firing",
    "receiver": "punchingfist",
    "alerts": [{
      "status": "firing",
      "labels": {
        "alertname": "PodCrashLooping",
        "severity": "critical",
        "namespace": "test-workloads",
        "pod": "crashloop-app"
      },
      "annotations": {
        "description": "Pod crashloop-app is crash looping",
        "summary": "Pod crash loop detected"
      },
      "startsAt": "2024-01-15T10:00:00Z"
    }]
  }'
```

## Step 6: View Results

Watch the operator logs to see the investigation in real-time:

```bash
# Follow operator logs
kubectl logs -n punching-fist -l app=punching-fist -f
```

You should see:
1. **Webhook received**: Alert ingestion
2. **Workflow triggered**: Investigation starts
3. **Agent execution**: LLM analyzing the issue
4. **Tool usage**: kubectl commands being executed
5. **Stdout sink**: Formatted investigation results

### Expected Output Format
```
========================================
🤖 PUNCHING FIST INVESTIGATION RESULTS
========================================

Alert: PodCrashLooping
Severity: critical
Time: 2024-01-15T10:05:23Z

----------------------------------------
INVESTIGATION SUMMARY:
----------------------------------------
The pod crashloop-app is failing due to...

----------------------------------------
DETAILED FINDINGS:
----------------------------------------
1. Pod has restarted 5 times in the last hour
2. Exit code 1 indicates application error
3. Logs show "Crashing!" message before exit

----------------------------------------
ROOT CAUSE ANALYSIS:
----------------------------------------
The application is designed to crash after 10 seconds...

----------------------------------------
RECOMMENDATIONS:
----------------------------------------
1. Check application configuration
2. Review recent code changes
3. Consider increasing health check grace period

========================================
Workflow: test-investigation
Duration: 15s
Status: completed
========================================
```

## Step 7: Verify Database State

Check that alerts are being stored:

```bash
# Access the SQLite database
kubectl exec -n punching-fist statefulset/punching-fist -it -- sqlite3 /data/punchingfist.db

# In SQLite prompt:
.tables
SELECT * FROM alerts;
SELECT * FROM workflows;
.quit
```

## Troubleshooting

### Operator not receiving webhooks
```bash
# Check service is accessible
kubectl get svc -n punching-fist
curl http://localhost:8080/health
```

### Workflow not triggering
```bash
# Check source controller logs
kubectl logs -n punching-fist -l app=punching-fist | grep source_controller

# Verify workflow exists
kubectl describe workflow test-investigation -n punching-fist
```

### Agent errors
```bash
# Check API key is set
kubectl get secret -n punching-fist punching-fist -o yaml | grep anthropic

# Test with mock provider
kubectl patch workflow test-investigation -n punching-fist --type merge -p '
spec:
  runtime:
    llmConfig:
      provider: "mock"'
```

## Test Scenarios

### 1. **Basic Alert Investigation**
- Send PodCrashLooping alert
- Verify agent investigates the pod
- Check stdout shows findings

### 2. **Resource Usage Investigation**
- Send HighCPUUsage or HighMemoryUsage alert
- Verify agent queries Prometheus metrics
- Check recommendations are appropriate

### 3. **Multiple Alerts**
- Send several alerts rapidly
- Verify each triggers separate workflow
- Check deduplication works

### 4. **Workflow Failure Recovery**
- Send alert with non-existent pod name
- Verify workflow handles gracefully
- Check error is logged appropriately

## Next Steps

Once basic testing is successful:

1. **Test with real Prometheus alerts**
   - Configure Prometheus to send to webhook
   - Create real alerting rules

2. **Add more sinks**
   - Slack integration
   - AlertManager annotation
   - PagerDuty escalation

3. **Production readiness**
   - Performance testing
   - Security hardening
   - Multi-namespace support

## Cleanup

```bash
# Remove test resources
kubectl delete -f examples/test-stdout-pipeline.yaml

# Uninstall operator
just test-cleanup

# Or manually
helm uninstall punching-fist -n punching-fist
kubectl delete namespace punching-fist test-workloads
```

## Success Criteria ✅

- [ ] Webhook receives alerts successfully
- [ ] Workflow triggers automatically
- [ ] Agent investigates using kubectl/promql
- [ ] Results appear in stdout sink
- [ ] Database contains alert records
- [ ] No errors in operator logs

---

**Congratulations!** 🎉 If all tests pass, you've successfully validated the Phase 1 MVP pipeline! 