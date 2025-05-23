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

# Deploy with custom OpenHands API key
# Usage: `just k8s-deploy-with-key YOUR_API_KEY`
k8s-deploy-with-key api_key:
    @echo "Deploying to Kubernetes with API key..."
    helm upgrade --install punching-fist charts/punching-fist \
        --values charts/punching-fist/values-local.yaml \
        --set openhands.apiKey={{api_key}} \
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

# Full local deployment with API key
# Usage: `just deploy-local-with-key YOUR_API_KEY`
deploy-local-with-key api_key: docker-build-local
    just k8s-deploy-with-key {{api_key}}
    just k8s-status
    @echo ""
    @echo "🎉 Deployment complete!"
    @echo "Use 'just k8s-port-forward' to access the operator at http://localhost:8080"
    @echo "Use 'just k8s-logs' to view logs"

# Show available commands
# Usage: `just help`
help:
    @just --list 