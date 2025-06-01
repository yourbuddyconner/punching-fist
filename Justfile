# Development commands for punching_fist_operator

# Run the operator server in development mode with live reload
# Usage: `just dev`
dev:
    cargo watch -x 'run -p punching-fist-operator'

# Run the operator server once without live reload
# Usage: `just run`
run:
    cargo run -p punching-fist-operator

# Build the project
# Usage: `just build`
build:
    cargo build

# Run tests
# Usage: `just test`
test:
    cargo test

# Check code without building
# Usage: `just check`
check:
    cargo check

# Clean build artifacts
# Usage: `just clean`
clean:
    cargo clean

# Docker commands for Kubernetes deployment

# Build Docker image for local deployment
# Usage: `just docker-build`
docker-build:
    docker build -t punching-fist:latest .

# Build and load image into Rancher Desktop
# Usage: `just docker-build-local`
docker-build-local: docker-build
    @echo "Image built and available in local Docker registry"
    @echo "Use 'just k8s-deploy' to deploy to Kubernetes"

# Kubernetes deployment commands

# Deploy to local Kubernetes cluster
# Usage: `just k8s-deploy`
k8s-deploy:
    @echo "Deploying to Kubernetes..."
    helm upgrade --install punching-fist charts/punching-fist \
        --values charts/punching-fist/values-local.yaml \
        --create-namespace \
        --namespace punching-fist-system

# Deploy with custom agent API key (Anthropic)
# Usage: `just k8s-deploy-with-key YOUR_ANTHROPIC_API_KEY`
k8s-deploy-with-key api_key:
    @echo "Deploying to Kubernetes with Anthropic API key..."
    helm upgrade --install punching-fist charts/punching-fist \
        --values charts/punching-fist/values-local.yaml \
        --set agent.provider=anthropic \
        --set agent.anthropicApiKey={{api_key}} \
        --create-namespace \
        --namespace punching-fist-system

# Deploy with OpenAI API key
# Usage: `just k8s-deploy-with-openai YOUR_OPENAI_API_KEY`
k8s-deploy-with-openai api_key:
    @echo "Deploying to Kubernetes with OpenAI API key..."
    helm upgrade --install punching-fist charts/punching-fist \
        --values charts/punching-fist/values-local.yaml \
        --set agent.provider=openai \
        --set agent.openaiApiKey={{api_key}} \
        --create-namespace \
        --namespace punching-fist-system

# Check deployment status
# Usage: `just k8s-status`
k8s-status:
    @echo "Checking deployment status..."
    kubectl get pods -n punching-fist-system
    kubectl get services -n punching-fist-system

# View operator logs
# Usage: `just k8s-logs`
k8s-logs:
    kubectl logs -f statefulset/punching-fist -n punching-fist-system

# Port forward to access the operator locally
# Usage: `just k8s-port-forward`
k8s-port-forward:
    @echo "Port forwarding to localhost:8080..."
    kubectl port-forward service/punching-fist 8080:8080 -n punching-fist-system

# Uninstall from Kubernetes
# Usage: `just k8s-uninstall`
k8s-uninstall:
    helm uninstall punching-fist -n punching-fist-system
    kubectl delete namespace punching-fist-system

# Full local deployment workflow
# Usage: `just deploy-local`
deploy-local: docker-build-local k8s-deploy k8s-status
    @echo ""
    @echo "🎉 Deployment complete!"
    @echo "Use 'just k8s-port-forward' to access the operator at http://localhost:8080"
    @echo "Use 'just k8s-logs' to view logs"

# Full local deployment with Anthropic API key
# Usage: `just deploy-local-with-key YOUR_ANTHROPIC_API_KEY`
deploy-local-with-key api_key: docker-build-local
    just k8s-deploy-with-key {{api_key}}
    just k8s-status
    @echo ""
    @echo "🎉 Deployment complete!"
    @echo "Use 'just k8s-port-forward' to access the operator at http://localhost:8080"
    @echo "Use 'just k8s-logs' to view logs"

# Full local deployment with OpenAI API key
# Usage: `just deploy-local-with-openai YOUR_OPENAI_API_KEY`
deploy-local-with-openai api_key: docker-build-local
    just k8s-deploy-with-openai {{api_key}}
    just k8s-status
    @echo ""
    @echo "🎉 Deployment complete!"
    @echo "Use 'just k8s-port-forward' to access the operator at http://localhost:8080"
    @echo "Use 'just k8s-logs' to view logs"

# === TEST ENVIRONMENT COMMANDS (Using current kubeconfig context) ===

# Check current Kubernetes context
# Usage: `just test-context`
test-context:
    @echo "Current Kubernetes context:"
    -@kubectl config current-context
    @echo ""
    @echo "Checking cluster connectivity..."
    @if kubectl cluster-info > /dev/null 2>&1; then \
        echo "✓ Cluster is accessible"; \
        echo ""; \
        kubectl cluster-info; \
    else \
        echo "✗ Cannot connect to Kubernetes cluster"; \
        echo ""; \
        echo "Possible solutions:"; \
        echo "1. If using Rancher Desktop: Make sure it's running and Kubernetes is enabled"; \
        echo "2. If using Docker Desktop: Enable Kubernetes in settings"; \
        echo "3. If using minikube: Run 'minikube start'"; \
        echo "4. Check your kubeconfig: 'kubectl config view'"; \
        exit 1; \
    fi

# Build operator for test deployment
# Usage: `just test-build-operator`
test-build-operator:
    @echo "Building operator Docker image..."
    cargo build --release
    docker build -t ttl.sh/punching-fist-operator:2h .
    docker push ttl.sh/punching-fist-operator:2h

# Update helm dependencies
# Usage: `just test-helm-deps`
test-helm-deps:
    @echo "Updating helm dependencies..."
    cd charts/punching-fist && helm dependency update

# Deploy CRDs to current k8s context using Helm
# Usage: `just test-deploy-operator`
test-deploy-operator: test-build-operator test-helm-deps
    @echo "Deploying operator with Helm to current context..."
    @echo "Current context: `kubectl config current-context`"
    @if [ -z "{{env_var_or_default("ANTHROPIC_API_KEY", "")}}" ]; then \
        echo "Error: ANTHROPIC_API_KEY is not set. Please create a .env file with this variable."; \
        exit 1; \
    fi
    helm upgrade --install punching-fist ./charts/punching-fist \
        --values ./charts/punching-fist/values-local.yaml \
        --set agent.anthropicApiKey={{env_var("ANTHROPIC_API_KEY")}} \
        --namespace punching-fist \
        --create-namespace \
        --wait
    @echo "Operator deployed successfully!"
    @echo "Note: CRDs are automatically installed by Helm"

# Run the test_rig_tools example against current context
# Usage: `just test-run-example`
test-run-example:
    @echo "Running test_rig_tools example..."
    cargo run --example test_rig_tools

# Port forward to operator in current context
# Usage: `just test-port-forward-operator`
test-port-forward-operator:
    @echo "Port forwarding to operator..."
    kubectl port-forward -n punching-fist svc/punching-fist 8080:8080

# Port forward to Prometheus in current context
# Usage: `just test-port-forward-prometheus`
test-port-forward-prometheus:
    @echo "Port forwarding to Prometheus..."
    kubectl port-forward -n punching-fist svc/prometheus-operated 9090:9090

# Show operator logs
# Usage: `just test-logs`
test-logs:
    kubectl logs -n punching-fist -l app=punching-fist -f

# Show test workload status
# Usage: `just test-workloads-status`
test-workloads-status:
    @echo "Test workloads in namespace test-workloads:"
    kubectl get pods -n test-workloads

# Full test deployment - sets up everything in current context
# Usage: `just test-deploy`
test-deploy: test-context test-deploy-operator
    @echo ""
    @echo "Test environment is ready!"
    @echo "Test resources deployed in namespace: test-workloads"
    @echo ""
    @echo "Available commands:"
    @echo "  just test-logs                    - View operator logs"
    @echo "  just test-port-forward-operator   - Access operator UI at http://localhost:8080"
    @echo "  just test-port-forward-prometheus - Access Prometheus at http://localhost:9090"
    @echo "  just test-workloads-status        - Check test workload status"

# Cleanup test deployment
# Usage: `just test-cleanup`
test-cleanup:
    @echo "Cleaning up test deployment..."
    helm uninstall punching-fist -n punching-fist || true
    kubectl delete namespace punching-fist || true
    kubectl delete namespace test-workloads || true

# Show available commands
# Usage: `just help`
help:
    @just --list

# === E2E TEST COMMANDS ===

# Deploy complete e2e test environment
# Usage: `just e2e-deploy`
e2e-deploy: test-deploy
    @echo ""
    @echo "🎉 E2E test environment deployed!"
    @echo ""
    @echo "Test pods will start generating alerts after a few minutes."
    @echo "Use the following commands to monitor the e2e flow:"
    @echo "  just e2e-status     - Check all components status"
    @echo "  just e2e-alerts     - View active Prometheus alerts"
    @echo "  just e2e-logs       - View punching-fist operator logs"
    @echo "  just e2e-trigger    - Manually trigger a test alert"

# Check e2e test status
# Usage: `just e2e-status`
e2e-status:
    @echo "=== Test Workloads ==="
    kubectl get pods -n test-workloads
    @echo ""
    @echo "=== Punching Fist Resources ==="
    kubectl get sources,workflows,sinks -n punching-fist
    @echo ""
    @echo "=== Alert Status ==="
    -@kubectl exec -n punching-fist prometheus-punching-fist-prometheus-s-prometheus-0 -- \
        promtool query instant http://localhost:9090 'ALERTS{namespace="test-workloads"}' | grep -E "(alertname|alertstate|severity)" || echo "No active test alerts"

# View active alerts
# Usage: `just e2e-alerts`
e2e-alerts:
    @echo "Checking Prometheus for active test alerts..."
    kubectl port-forward -n punching-fist svc/punching-fist-prometheus-s-prometheus 9090:9090 &
    @sleep 2
    @curl -s http://localhost:9090/api/v1/alerts | jq '.data.alerts[] | select(.labels.namespace=="test-workloads") | {alert: .labels.alertname, state: .state, severity: .labels.severity, pod: .labels.pod}'
    @pkill -f "port-forward.*9090:9090" || true

# View operator logs for e2e testing
# Usage: `just e2e-logs`
e2e-logs:
    @echo "Viewing punching-fist operator logs..."
    kubectl logs -f statefulset/punching-fist -n punching-fist

# Port forward to AlertManager
# Usage: `just e2e-alertmanager`
e2e-alertmanager:
    @echo "Port forwarding to AlertManager at http://localhost:9093..."
    kubectl port-forward -n punching-fist svc/punching-fist-prometheus-s-alertmanager 9093:9093

# Manually trigger a test alert via AlertManager API
# Usage: `just e2e-trigger`
e2e-trigger:
    @echo "Sending test alert to AlertManager..."
    @kubectl port-forward -n punching-fist svc/punching-fist-prometheus-s-alertmanager 9093:9093 &
    @sleep 2
    @curl -XPOST http://localhost:9093/api/v1/alerts \
        -H "Content-Type: application/json" \
        -d '[{"labels": {"alertname": "TestPodCrashLooping", "severity": "critical", "namespace": "test-workloads", "pod": "manual-test-pod"}, "annotations": {"description": "Manual test alert: Pod manual-test-pod is crash looping", "summary": "Test pod is crash looping"}}]'
    @pkill -f "port-forward.*9093:9093" || true
    @echo ""
    @echo "✓ Test alert sent! Check logs with 'just e2e-logs'"

# View workflow executions
# Usage: `just e2e-workflows`
e2e-workflows:
    @echo "=== Workflow Executions ==="
    kubectl get workflowexecutions -n punching-fist -o wide
    @echo ""
    @echo "To view details of a specific execution:"
    @echo "kubectl describe workflowexecution <name> -n punching-fist"

# Restart crashloop pod to trigger alerts
# Usage: `just e2e-restart-crashloop`
e2e-restart-crashloop:
    @echo "Deleting crashloop pod to trigger restart alerts..."
    kubectl delete pod crashloop-app -n test-workloads --force --grace-period=0 || true
    kubectl apply -f charts/punching-fist/templates/tests/test-pods.yaml
    @echo "✓ Crashloop pod restarted. Alerts should fire in ~2-5 minutes."

# Clean up e2e test resources only (keep operator)
# Usage: `just e2e-clean-tests`
e2e-clean-tests:
    @echo "Cleaning up test resources..."
    kubectl delete namespace test-workloads || true
    kubectl delete sources,workflows,sinks -l test-resources=true -n punching-fist || true

# Full e2e cleanup
# Usage: `just e2e-cleanup`
e2e-cleanup: test-cleanup
    @echo "E2E test environment cleaned up" 