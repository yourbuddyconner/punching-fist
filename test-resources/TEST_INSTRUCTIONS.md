# Punching Fist Test Instructions

## Quick Start

1. **Deploy with test resources enabled:**
   ```bash
   helm install punching-fist ./charts/punching-fist \
     --namespace punching-fist \
     --create-namespace \
     --set testResources.enabled=true \
     --set testResources.pipeline.enabled=true \
     --set agent.anthropicApiKey=$ANTHROPIC_API_KEY
   ```

2. **Port-forward the operator:**
   ```bash
   kubectl port-forward -n punching-fist svc/punching-fist 8080:8080
   ```

3. **Send test alerts:**
   ```bash
   ./test-resources/send-test-alert.sh
   ```

## Important Notes

### Webhook Configuration
The test webhook is configured with specific filters:
- **Path:** `/webhook/test-alerts`
- **Allowed alert names:** 
  - `TestPodCrashLooping`
  - `TestPodHighCPUUsage`
  - `TestPodHighMemoryUsage`
  - `TestPodNotReady`
- **Allowed severities:** `critical`, `warning`

### What Happens When You Send an Alert

1. **Alert Reception:** The operator receives the webhook at `/webhook/test-alerts`
2. **Filtering:** Only alerts matching the configured filters are processed
3. **Alert Storage:** Valid alerts are saved to the SQLite database
4. **Source Event:** A source event is created to track the webhook reception
5. **Workflow Trigger:** The configured workflow would be triggered (when fully implemented)

### Checking Logs

Watch the operator logs to see alert processing:
```bash
kubectl logs -n punching-fist -l app=punching-fist -f
```

You should see messages like:
```
INFO punching_fist_operator::server::routes: Received AlertManager webhook on path: /test-alerts
INFO punching_fist_operator::sources::webhook: Processing AlertManager webhook for source punching-fist-test-webhook with 1 alerts
INFO punching_fist_operator::sources::webhook: Created new alert <uuid> with fingerprint <hash>
INFO punching_fist_operator::server::routes: Successfully processed 1 alerts
```

### Troubleshooting

If alerts are being filtered out:
1. Check that test resources are enabled in your Helm values
2. Verify the Source CRD exists: `kubectl get sources -n punching-fist`
3. Ensure you're using the correct alert names (prefixed with "Test")
4. Make sure the severity is either "critical" or "warning"

### Database Check

To verify alerts are being stored:
```bash
kubectl exec -n punching-fist punching-fist-0 -- sqlite3 /data/punching-fist.db \
  "SELECT id, alert_name, severity, status FROM alerts;"
``` 