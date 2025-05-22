# Punching Fist Operator Design Document

## System Architecture

### High-Level Overview

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Prometheus     │     │  Punching Fist  │     │  Kubernetes     │
│  AlertManager   │────▶│    Operator     │────▶│     Cluster     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                              │
                              ▼
                       ┌─────────────────┐
                       │    OpenHands    │
                       │    (Headless)   │
                       └─────────────────┘
```

### Core Components

1. **Alert Receiver System**
   - Modular design supporting multiple receiver types
   - Currently implements Prometheus webhook receiver
   - Extensible for future receiver types (e.g., Slack, PagerDuty)
   - Handles alert validation and transformation

2. **Task Scheduler**
   - Manages scheduled maintenance tasks
   - Implements cron-like scheduling
   - Handles task prioritization and concurrency

3. **OpenHands Integration**
   - Manages communication with OpenHands API
   - Handles task processing and response parsing
   - Implements retry logic and error handling

4. **Kubernetes Client**
   - Manages cluster operations
   - Implements RBAC and service account authentication
   - Handles resource management and cleanup

## Technical Implementation

### Rust Project Structure

```
punching-fist/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── server/
│   │   ├── mod.rs
│   │   ├── receivers/
│   │   │   ├── mod.rs
│   │   │   ├── prometheus.rs
│   │   │   └── traits.rs
│   │   └── routes.rs
│   ├── kubernetes/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── resources.rs
│   ├── openhands/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── tasks.rs
│   └── scheduler/
│       ├── mod.rs
│       └── task.rs
└── tests/
    └── integration/
```

### Key Dependencies

```toml
[dependencies]
axum = "0.7"
tokio = { version = "1.0", features = ["full"] }
kube = "0.88"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tower = "0.4"
tracing = "0.1"
```

### Alert Receiver System

#### Receiver Trait

```rust
#[async_trait]
pub trait AlertReceiver: Send + Sync {
    async fn handle_alert(&self, alert: Alert) -> Result<()>;
    fn validate_alert(&self, alert: &Alert) -> Result<()>;
    fn transform_alert(&self, alert: Alert) -> Result<Task>;
}
```

#### Prometheus Webhook Receiver

The Prometheus webhook receiver implements the AlertReceiver trait and handles Prometheus AlertManager's webhook format:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusAlert {
    pub version: String,
    pub group_key: String,
    pub truncated_alerts: Option<i32>,
    pub status: String,
    pub receiver: String,
    pub group_labels: HashMap<String, String>,
    pub common_labels: HashMap<String, String>,
    pub common_annotations: HashMap<String, String>,
    pub external_url: String,
    pub alerts: Vec<PrometheusAlertDetail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PrometheusAlertDetail {
    pub status: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub generator_url: String,
    pub fingerprint: String,
}

pub struct PrometheusReceiver {
    config: PrometheusConfig,
}

impl PrometheusReceiver {
    pub fn new(config: PrometheusConfig) -> Self {
        Self { config }
    }

    fn validate_alert(&self, alert: &PrometheusAlert) -> Result<()> {
        // Validate alert format and required fields
        if alert.version != "4" {
            return Err(Error::InvalidAlert("Unsupported alert version".into()));
        }
        Ok(())
    }

    fn transform_alert(&self, alert: PrometheusAlert) -> Result<Task> {
        // Transform Prometheus alert format to internal Task format
        let task = Task {
            id: Uuid::new_v4().to_string(),
            prompt: format!(
                "Handle the following Kubernetes alert:\n\
                Group: {}\n\
                Status: {}\n\
                Labels: {:?}\n\
                Annotations: {:?}",
                alert.group_key,
                alert.status,
                alert.common_labels,
                alert.common_annotations
            ),
            model: None,
            max_retries: Some(3),
            timeout: Some(300),
            resources: TaskResources {
                cpu_limit: "500m".to_string(),
                memory_limit: "512Mi".to_string(),
                cpu_request: "100m".to_string(),
                memory_request: "128Mi".to_string(),
            },
        };
        Ok(task)
    }
}

#[async_trait]
impl AlertReceiver for PrometheusReceiver {
    async fn handle_alert(&self, alert: PrometheusAlert) -> Result<()> {
        self.validate_alert(&alert)?;
        let task = self.transform_alert(alert)?;
        // Schedule the task
        Ok(())
    }
}
```

#### Configuration

The Prometheus webhook receiver can be configured through the operator's configuration:

```yaml
receivers:
  prometheus:
    enabled: true
    send_resolved: true
    max_alerts: 0
    timeout: 0s
    http_config:
      basic_auth:
        username: ""
        password: ""
      bearer_token: ""
      tls_config:
        ca_file: ""
        cert_file: ""
        key_file: ""
```

### Webhook Server Implementation

```rust
use axum::{
    extract::Json,
    response::IntoResponse,
    routing::post,
    Router,
};

pub async fn alert_handler(
    State(receiver): State<Arc<dyn AlertReceiver>>,
    Json(alert): Json<PrometheusAlert>,
) -> impl IntoResponse {
    match receiver.handle_alert(alert).await {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            error!("Error handling alert: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
```

### Kubernetes Integration

```rust
use kube::{
    api::{Api, Resource},
    Client,
};

pub struct KubeClient {
    client: Client,
    namespace: String,
}

impl KubeClient {
    pub async fn new() -> Result<Self, Error> {
        let client = Client::try_default().await?;
        Ok(Self {
            client,
            namespace: std::env::var("NAMESPACE").unwrap_or_default(),
        })
    }

    pub async fn execute_task(&self, task: Task) -> Result<(), Error> {
        // Implement task execution logic
    }
}
```

### OpenHands Integration

```rust
pub struct OpenHandsClient {
    api_key: String,
    client: reqwest::Client,
}

impl OpenHandsClient {
    pub async fn process_task(&self, task: Task) -> Result<TaskResult, Error> {
        // Implement OpenHands API integration
    }
}
```

### Task Job Management

The operator creates Kubernetes Jobs for each OpenHands task, providing resource management, visibility, and retry capabilities. Each job runs OpenHands in headless mode to execute the maintenance tasks.

```rust
use kube::{
    api::{Api, Resource},
    Client,
};

pub struct TaskJobManager {
    client: Client,
    namespace: String,
}

impl TaskJobManager {
    pub async fn create_task_job(&self, task: Task) -> Result<(), Error> {
        let job = Job {
            metadata: ObjectMeta {
                name: Some(format!("openhands-task-{}", task.id)),
                namespace: Some(self.namespace.clone()),
                labels: Some({
                    let mut labels = HashMap::new();
                    labels.insert("app.kubernetes.io/name".to_string(), "punching-fist".to_string());
                    labels.insert("task.type".to_string(), "openhands".to_string());
                    labels.insert("task.id".to_string(), task.id.clone());
                    labels
                }),
                ..Default::default()
            },
            spec: Some(JobSpec {
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        containers: vec![Container {
                            name: "openhands-task".to_string(),
                            image: Some("docker.all-hands.dev/all-hands-ai/openhands:0.39".to_string()),
                            command: Some(vec!["python".to_string()]),
                            args: Some(vec![
                                "-m".to_string(),
                                "openhands.core.main".to_string(),
                                "-t".to_string(),
                                task.prompt,
                            ]),
                            env: Some(vec![
                                EnvVar {
                                    name: "LLM_API_KEY".to_string(),
                                    value: Some(self.api_key.clone()),
                                    ..Default::default()
                                },
                                EnvVar {
                                    name: "LLM_MODEL".to_string(),
                                    value: Some("anthropic/claude-3-7-sonnet-20250219".to_string()),
                                    ..Default::default()
                                },
                                EnvVar {
                                    name: "LOG_ALL_EVENTS".to_string(),
                                    value: Some("true".to_string()),
                                    ..Default::default()
                                },
                                EnvVar {
                                    name: "SANDBOX_RUNTIME_CONTAINER_IMAGE".to_string(),
                                    value: Some("docker.all-hands.dev/all-hands-ai/runtime:0.39-nikolaik".to_string()),
                                    ..Default::default()
                                },
                            ]),
                            volume_mounts: Some(vec![
                                VolumeMount {
                                    name: "docker-sock".to_string(),
                                    mount_path: "/var/run/docker.sock".to_string(),
                                    ..Default::default()
                                },
                                VolumeMount {
                                    name: "openhands-state".to_string(),
                                    mount_path: "/.openhands-state".to_string(),
                                    ..Default::default()
                                },
                            ]),
                            resources: Some(ResourceRequirements {
                                limits: Some({
                                    let mut limits = HashMap::new();
                                    limits.insert("cpu".to_string(), "500m".to_string());
                                    limits.insert("memory".to_string(), "512Mi".to_string());
                                    limits
                                }),
                                requests: Some({
                                    let mut requests = HashMap::new();
                                    requests.insert("cpu".to_string(), "100m".to_string());
                                    requests.insert("memory".to_string(), "128Mi".to_string());
                                    requests
                                }),
                            }),
                            security_context: Some(SecurityContext {
                                privileged: Some(true), // Required for Docker-in-Docker
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                        volumes: Some(vec![
                            Volume {
                                name: "docker-sock".to_string(),
                                host_path: Some(HostPathVolumeSource {
                                    path: "/var/run/docker.sock".to_string(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            Volume {
                                name: "openhands-state".to_string(),
                                empty_dir: Some(EmptyDirVolumeSource {
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        ]),
                        restart_policy: Some("OnFailure".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                backoff_limit: Some(3),
                ttl_seconds_after_finished: Some(3600), // Clean up after 1 hour
                ..Default::default()
            }),
            ..Default::default()
        };

        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);
        jobs.create(&PostParams::default(), &job).await?;
        Ok(())
    }

    pub async fn monitor_task_job(&self, task_id: String) -> Result<JobStatus, Error> {
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);
        let job = jobs.get(&format!("openhands-task-{}", task_id)).await?;
        Ok(job.status.unwrap_or_default())
    }
}
```

#### Job Template

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: openhands-task-{{ .Values.task.id }}
  namespace: {{ .Release.Namespace }}
  labels:
    app.kubernetes.io/name: punching-fist
    task.type: openhands
    task.id: {{ .Values.task.id }}
spec:
  template:
    spec:
      containers:
      - name: openhands-task
        image: docker.all-hands.dev/all-hands-ai/openhands:0.39
        command: ["python"]
        args:
        - "-m"
        - "openhands.core.main"
        - "-t"
        - {{ .Values.task.prompt | quote }}
        env:
        - name: LLM_API_KEY
          valueFrom:
            secretKeyRef:
              name: openhands-secrets
              key: api-key
        - name: LLM_MODEL
          value: "anthropic/claude-3-7-sonnet-20250219"
        - name: LOG_ALL_EVENTS
          value: "true"
        - name: SANDBOX_RUNTIME_CONTAINER_IMAGE
          value: "docker.all-hands.dev/all-hands-ai/runtime:0.39-nikolaik"
        volumeMounts:
        - name: docker-sock
          mountPath: /var/run/docker.sock
        - name: openhands-state
          mountPath: /.openhands-state
        resources:
          limits:
            cpu: {{ .Values.task.resources.limits.cpu }}
            memory: {{ .Values.task.resources.limits.memory }}
          requests:
            cpu: {{ .Values.task.resources.requests.cpu }}
            memory: {{ .Values.task.resources.requests.memory }}
        securityContext:
          privileged: true  # Required for Docker-in-Docker
      volumes:
      - name: docker-sock
        hostPath:
          path: /var/run/docker.sock
      - name: openhands-state
        emptyDir: {}
      restartPolicy: OnFailure
  backoffLimit: {{ .Values.task.backoffLimit }}
  ttlSecondsAfterFinished: {{ .Values.task.ttlSecondsAfterFinished }}
```

#### Task Configuration

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub prompt: String,
    pub model: Option<String>,
    pub max_retries: Option<i32>,
    pub timeout: Option<i32>,
    pub resources: TaskResources,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResources {
    pub cpu_limit: String,
    pub memory_limit: String,
    pub cpu_request: String,
    pub memory_request: String,
}
```

#### Task Status Tracking

The operator tracks task status through Job conditions and annotations:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub phase: TaskPhase,
    pub start_time: Option<DateTime<Utc>>,
    pub completion_time: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub retry_count: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Retrying,
}
```

#### Task Monitoring

The operator provides monitoring capabilities for OpenHands tasks:

```rust
impl TaskJobManager {
    pub async fn get_task_metrics(&self) -> Result<TaskMetrics, Error> {
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);
        let jobs_list = jobs.list(&ListParams::default()).await?;
        
        let mut metrics = TaskMetrics::default();
        for job in jobs_list.items {
            if let Some(status) = job.status {
                match status.phase.as_deref() {
                    Some("Succeeded") => metrics.succeeded += 1,
                    Some("Failed") => metrics.failed += 1,
                    Some("Running") => metrics.running += 1,
                    _ => metrics.pending += 1,
                }
            }
        }
        Ok(metrics)
    }
}
```

#### Prometheus Metrics

The operator exposes Prometheus metrics for task monitoring:

```rust
use prometheus::{Counter, Gauge, Histogram, Registry};

pub struct TaskMetrics {
    pub tasks_total: Counter,
    pub tasks_running: Gauge,
    pub tasks_succeeded: Counter,
    pub tasks_failed: Counter,
    pub task_duration: Histogram,
}

impl TaskMetrics {
    pub fn new(registry: &Registry) -> Self {
        Self {
            tasks_total: Counter::new(
                "openhands_tasks_total",
                "Total number of OpenHands tasks created"
            ).unwrap(),
            tasks_running: Gauge::new(
                "openhands_tasks_running",
                "Number of OpenHands tasks currently running"
            ).unwrap(),
            tasks_succeeded: Counter::new(
                "openhands_tasks_succeeded_total",
                "Total number of successfully completed OpenHands tasks"
            ).unwrap(),
            tasks_failed: Counter::new(
                "openhands_tasks_failed_total",
                "Total number of failed OpenHands tasks"
            ).unwrap(),
            task_duration: Histogram::with_opts(
                HistogramOpts::new(
                    "openhands_task_duration_seconds",
                    "Duration of OpenHands tasks in seconds"
                )
            ).unwrap(),
        }
    }
}
```

## Security Considerations

1. **Authentication**
   - Service account-based authentication for Kubernetes operations
   - API key management for OpenHands integration
   - WebSocket authentication using JWT tokens

2. **Authorization**
   - RBAC rules for Kubernetes operations

## Monitoring and Observability

1. **Metrics**
   - Prometheus metrics for:
     - WebSocket connections
     - Task execution times
     - API call latencies
     - Error rates

2. **Logging**
   - Structured logging with tracing
   - Log levels configuration
   - Audit trail for operations

3. **Health Checks**
   - Liveness probe
   - Readiness probe
   - Startup probe

## Deployment

### Kubernetes Resources

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: punching-fist
spec:
  replicas: 1
  template:
    spec:
      serviceAccountName: punching-fist
      containers:
      - name: operator
        image: punching-fist:latest
        ports:
        - containerPort: 8080
        env:
        - name: OPENHANDS_API_KEY
          valueFrom:
            secretKeyRef:
              name: punching-fist-secrets
              key: openhands-api-key
```

### Service Account

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: punching-fist
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: punching-fist
rules:
- apiGroups: [""]
  resources: ["pods", "services", "configmaps"]
  verbs: ["get", "list", "watch", "create", "update", "delete"]
```

### Helm Chart

The operator is packaged as a Helm chart for easy deployment and configuration. The chart structure follows Helm best practices:

```
punching-fist/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── _helpers.tpl
│   ├── deployment.yaml
│   ├── service.yaml
│   ├── serviceaccount.yaml
│   ├── clusterrole.yaml
│   ├── clusterrolebinding.yaml
│   ├── configmap.yaml
│   ├── secret.yaml
│   └── NOTES.txt
└── crds/
    └── maintenance-tasks.yaml
```

#### Chart.yaml
```yaml
apiVersion: v2
name: punching-fist
description: A Kubernetes operator for AI-powered cluster maintenance
type: application
version: 0.1.0
appVersion: "1.0.0"
```

#### values.yaml
```yaml
# Global settings
global:
  image:
    repository: punching-fist
    tag: latest
    pullPolicy: IfNotPresent

# Operator configuration
operator:
  replicaCount: 1
  resources:
    limits:
      cpu: 500m
      memory: 512Mi
    requests:
      cpu: 100m
      memory: 128Mi

# Server configuration
server:
  port: 8080
  host: "0.0.0.0"

# OpenHands configuration
openhands:
  enabled: true
  apiKey: ""  # Set via --set or secret
  model: "anthropic/claude-3-7-sonnet-20250219"

# Prometheus configuration
prometheus:
  enabled: true
  serviceMonitor:
    enabled: true
    interval: 15s

# Security configuration
security:
  serviceAccount:
    create: true
    name: punching-fist
  rbac:
    create: true
    rules:
      - apiGroups: [""]
        resources: ["pods", "services", "configmaps"]
        verbs: ["get", "list", "watch", "create", "update", "delete"]

# Pod security context
podSecurityContext:
  fsGroup: 1000
  runAsUser: 1000
  runAsNonRoot: true

# Container security context
containerSecurityContext:
  allowPrivilegeEscalation: false
  readOnlyRootFilesystem: true
  runAsNonRoot: true
  capabilities:
    drop:
      - ALL
```

#### Key Templates

1. **deployment.yaml**
   - Configures the operator deployment
   - Sets up environment variables
   - Configures resource limits
   - Sets up health checks

2. **service.yaml**
   - Exposes the WebSocket server
   - Configures service type and ports

3. **configmap.yaml**
   - Stores operator configuration
   - Manages feature flags
   - Configures logging levels

4. **secret.yaml**
   - Manages sensitive data
   - Stores API keys
   - Handles TLS certificates

#### Usage Examples

1. **Basic Installation**
```bash
helm install punching-fist ./punching-fist \
  --namespace punching-fist \
  --create-namespace
```

2. **Custom Configuration**
```bash
helm install punching-fist ./punching-fist \
  --namespace punching-fist \
  --set openhands.apiKey=your-api-key \
  --set operator.replicaCount=2 \
  --set server.port=9090
```

3. **Production Deployment**
```bash
helm install punching-fist ./punching-fist \
  --namespace punching-fist \
  --values values-production.yaml \
  --set openhands.apiKey=your-api-key
```

#### values-production.yaml
```yaml
operator:
  replicaCount: 3
  resources:
    limits:
      cpu: 1000m
      memory: 1Gi
    requests:
      cpu: 500m
      memory: 512Mi

prometheus:
  serviceMonitor:
    enabled: true
    interval: 10s

security:
  podSecurityContext:
    fsGroup: 1000
    runAsUser: 1000
    runAsNonRoot: true
  containerSecurityContext:
    allowPrivilegeEscalation: false
    readOnlyRootFilesystem: true
    runAsNonRoot: true
    capabilities:
      drop:
        - ALL
```

## Future Considerations

1. **Scalability**
   - Horizontal scaling support
   - Task queue implementation
   - Resource optimization

2. **Features**
   - Custom resource definitions for tasks
   - Plugin system for custom actions
   - Multi-cluster support

3. **Integration**
   - Additional alert manager support
   - Custom metrics collection
   - External API integration

## Development Workflow

1. **Local Development**
   - Minikube setup
   - Development environment configuration
   - Testing utilities

2. **CI/CD**
   - GitHub Actions workflow
   - Container image building
   - Automated testing

3. **Release Process**
   - Version management
   - Changelog maintenance
   - Release automation 