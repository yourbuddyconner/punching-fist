# Punching Fist Operator (👊🤖) Design Document

## Overview

Punching Fist Operator is a Kubernetes operator written in Rust that serves as an **intelligent incident response middleware**. By positioning itself between alert sources and traditional destinations (PagerDuty, on-call engineers), it leverages AI-powered automation to dramatically improve incident response KPIs through intelligent triage, context enrichment, and automated resolution.

## Core Vision

The operator provides **LLM agents with a comprehensive tooling sandbox** for immediate alert investigation. Instead of just routing alerts to humans, agents can execute the same investigative steps a human engineer would take - checking logs, querying metrics, inspecting cluster state - but instantly and with perfect recall of similar past issues.

## Value Proposition

**Traditional Flow:**
```
Alert → Human Engineer → Manual Investigation → Resolution
```

**Punching Fist Flow:**
```
Alert → LLM Agent → Automated Investigation (logs, metrics, cluster state) → Resolution/Enriched Escalation
```

**Key Benefits:**
- **Immediate Investigation**: No waiting for human availability - agent starts investigating instantly
- **Comprehensive Analysis**: Agent can check logs, metrics, and cluster state simultaneously  
- **Perfect Memory**: Never forgets past solutions or patterns
- **24/7 Availability**: Consistent response time regardless of time/day
- **Rich Context**: When escalation is needed, provides complete investigation results

## Architecture

### Core Abstraction: Source → Workflow → Sink

The operator follows a clean event-driven architecture where **Sources** trigger **Workflows** that output to **Sinks**. This separation of concerns enables composability and flexible integrations.

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│     SOURCES     │───▶│    WORKFLOWS     │───▶│     SINKS       │
│                 │    │                  │    │                 │
│ • AlertManager  │    │ • Agent Tasks    │    │ • Slack         │
│ • Chat Commands │    │ • LLM Loops      │    │ • AlertManager  │
│ • Cron/Schedule │    │ • CLI Execution  │    │ • Metrics       │
│ • API Calls     │    │ • Approval Gates │    │ • Tickets       │
│ • Chained Flows │    │ • Conditionals   │    │ • Databases     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │
                                ▼
                       ┌──────────────────┐
                       │  LLM Runtime     │
                       │  (Local/Cloud)   │
                       └──────────────────┘
                                │
                                ▼
                       ┌──────────────────┐
                       │  Kubernetes API  │
                       │  + CLI Tools     │
                       └──────────────────┘
```

### Core Components

#### 1. **Source Handlers**
- **Webhook Server**: Receives alerts from AlertManager/Prometheus
- **Chat Bot**: Processes Slack commands and mentions
- **Scheduler**: Handles cron-like triggers for maintenance
- **API Server**: External systems can trigger workflows
- **Event Watchers**: Kubernetes events, file changes, etc.

#### 2. **Workflow Engine**
- Watches Custom Resources (Source, Workflow, Sink)
- Manages workflow execution and state transitions
- Handles agent task lifecycle within workflows
- Orchestrates Source → Workflow → Sink flows

### 3. **State Management Layer**
- **Database Support**: Postgres (primary) and SQLite (development/testing)
- **Core Tables**:
  - **Alerts**: Complete alert lifecycle tracking
  - **Workflows**: Execution history and outcomes
  - **Incidents**: Grouped alerts and resolution patterns
  - **Knowledge Base**: Learned patterns and successful resolutions
- **Stored Data**:
  - Alert ingestion → triage → resolution timeline
  - AI analysis results and confidence scores
  - Human intervention patterns and outcomes
  - Cross-alert correlation and incident grouping

#### 4. **LLM Runtime Integration**
- **Powered by Rig**: Rust library for ergonomic LLM integration
- **Provider Abstraction**: Unified interface across LLM providers:
  - **Local LLMs**: Via Rig's local provider support (primary target)
  - **Cloud Providers**: Claude (Anthropic), OpenAI, Cohere, Gemini
  - **Open Models**: Together.ai integration for Llama, Mistral, etc.
- **Agent Framework**: Built on Rig's completion and tool-use patterns
- **Context Management**: Leverages Rig's built-in context handling
- **Prompt Engineering**: Structured prompts using Rig's builder patterns

## Database Schema

### Core Tables for Incident Response Management

#### **Alerts Table**
```sql
CREATE TABLE alerts (
    id UUID PRIMARY KEY,
    external_id VARCHAR(255) UNIQUE,  -- AlertManager alert ID
    fingerprint VARCHAR(255),         -- Alert deduplication key
    status VARCHAR(50) NOT NULL,      -- received, triaging, resolved, escalated
    severity VARCHAR(20) NOT NULL,    -- critical, warning, info
    alert_name VARCHAR(255) NOT NULL,
    summary TEXT,
    description TEXT,
    labels JSONB,                     -- Alert labels from monitoring system
    annotations JSONB,                -- Alert annotations
    source_id UUID REFERENCES sources(id),
    workflow_id UUID REFERENCES workflows(id),
    
    -- AI Analysis
    ai_analysis JSONB,                -- AI triage results
    ai_confidence FLOAT,              -- Confidence score (0-1)
    auto_resolved BOOLEAN DEFAULT FALSE,
    
    -- Timing Metrics
    received_at TIMESTAMP NOT NULL,
    triage_started_at TIMESTAMP,
    triage_completed_at TIMESTAMP,
    resolved_at TIMESTAMP,
    escalated_at TIMESTAMP,
    
    -- Human Interaction
    escalated_to VARCHAR(255),        -- PagerDuty, person, etc.
    human_actions JSONB,              -- Manual steps taken
    resolution_notes TEXT,
    
    -- Relationships
    incident_id UUID REFERENCES incidents(id),
    parent_alert_id UUID REFERENCES alerts(id),  -- For related alerts
    
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

-- Indexes for common queries
CREATE INDEX idx_alerts_status ON alerts(status);
CREATE INDEX idx_alerts_severity ON alerts(severity);
CREATE INDEX idx_alerts_fingerprint ON alerts(fingerprint);
CREATE INDEX idx_alerts_received_at ON alerts(received_at);
CREATE INDEX idx_alerts_incident_id ON alerts(incident_id);
```

#### **Incidents Table**
```sql
CREATE TABLE incidents (
    id UUID PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    status VARCHAR(50) NOT NULL,      -- open, investigating, resolved
    severity VARCHAR(20) NOT NULL,
    incident_commander VARCHAR(255),  -- Assigned human
    
    -- Metrics
    alerts_count INTEGER DEFAULT 0,
    mttr_seconds INTEGER,             -- Mean Time to Resolution
    
    -- AI Insights
    ai_summary JSONB,                 -- AI-generated incident summary
    root_cause_analysis TEXT,
    lessons_learned TEXT,
    
    created_at TIMESTAMP DEFAULT NOW(),
    resolved_at TIMESTAMP,
    updated_at TIMESTAMP DEFAULT NOW()
);
```

#### **Alert Relationships Table**
```sql
CREATE TABLE alert_relationships (
    id UUID PRIMARY KEY,
    parent_alert_id UUID REFERENCES alerts(id),
    child_alert_id UUID REFERENCES alerts(id),
    relationship_type VARCHAR(50),    -- duplicate, related, caused_by
    confidence FLOAT,                 -- AI confidence in relationship
    created_at TIMESTAMP DEFAULT NOW()
);
```
- **Primary**: Slack integration
- **Purpose**: Human-AI collaboration for approval gates and updates
- **Features**: Real-time task status, approval requests, manual intervention

## Custom Resource Definitions

### Source
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Source
metadata:
  name: alertmanager-critical
spec:
  type: webhook
  config:
    path: "/webhook/alerts"
    filters:
      severity: ["critical", "warning"]
      alertname: ["HighCPUUsage", "PodCrashLooping"]
  triggerWorkflow: "alert-triage-workflow"
  context:
    # Additional context to pass to workflow
    runbookRepo: "https://github.com/company/runbooks"
```

### Workflow (LLM Agent Investigation)
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Workflow
metadata:
  name: agent-alert-investigation
spec:
  runtime:
    image: "punchingfist/runtime:v1.0.0"
    llmConfig:
      provider: "local"
      endpoint: "http://llm-service:8080"
      model: "llama-3.1-70b"
    environment:
      PROMETHEUS_URL: "http://prometheus:9090"
      
  steps:
    - name: "initial-context"
      type: "cli"
      command: |
        echo "Alert: {{ .source.data.alert.alertname }}"
        echo "Summary: {{ .source.data.alert.summary }}"
        echo "Labels: {{ .source.data.alert.labels | toJSON }}"
    
    - name: "agent-investigation"
      type: "agent"
      goal: |
        I need to investigate this alert: {{ .source.data.alert.summary }}
        
        Available information:
        - Alert name: {{ .source.data.alert.alertname }}
        - Affected service: {{ .source.data.alert.labels.service }}
        - Namespace: {{ .source.data.alert.labels.namespace }}
        - Severity: {{ .source.data.alert.labels.severity }}
        
        Please investigate thoroughly and determine:
        1. What exactly is wrong?
        2. What caused this issue?
        3. Can I fix it automatically?
        4. If not, what should the on-call engineer know?
        
        Start by gathering relevant information, then analyze it step by step.
      
      tools:
        - name: "kubectl"
          description: "Kubernetes command line tool for cluster inspection"
        - name: "promql"
          description: "Query Prometheus metrics using PromQL"
          endpoint: "{{ .env.PROMETHEUS_URL }}"
        - name: "curl"
          description: "HTTP client for API calls and health checks"
        - name: "debug-pod"
          description: "Custom script for comprehensive pod debugging"
          command: "/usr/local/bin/debug-pod.sh"
      
      maxIterations: 15
      timeoutMinutes: 10
      
      # The agent will autonomously use these tools to investigate
      # Example agent reasoning:
      # 1. "Let me check the pod status: kubectl get pods -n {{ namespace }}"
      # 2. "I see pods are crash looping, let me check logs: kubectl logs..."
      # 3. "Logs show OOM errors, let me check resource usage: promql query..."
      # 4. "CPU/Memory metrics show spike, let me check if this is normal..."
      
    - name: "resolution-attempt"
      type: "conditional" 
      condition: "{{ .steps.agent-investigation.result.can_auto_fix }}"
      agent:
        goal: |
          Based on my investigation, I determined I can attempt to fix this automatically.
          Issue: {{ .steps.agent-investigation.result.problem_summary }}
          Proposed fix: {{ .steps.agent-investigation.result.fix_command }}
          
          Execute the fix and verify it worked.
        tools: ["kubectl"]
        maxIterations: 5
        approvalRequired: true  # Safety gate for destructive actions
  
  outputs:
    - name: "investigation_summary"
      value: "{{ .steps.agent-investigation.result.summary }}"
    - name: "root_cause"
      value: "{{ .steps.agent-investigation.result.root_cause }}"
    - name: "auto_fixed"
      value: "{{ .steps.resolution-attempt.success | default false }}"
    - name: "escalation_context"
      value: "{{ .steps.agent-investigation.result.escalation_notes }}"
  
  sinks:
    - name: "slack-investigation-results"
    - name: "alertmanager-annotation"
    - name: "pagerduty-escalation"
      condition: "{{ not .outputs.auto_fixed }}"
```

### Sink
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Sink
metadata:
  name: slack-ops-channel
spec:
  type: slack
  config:
    channel: "#ops-alerts"
    botToken: "xoxb-secret"
    template: |
      🚨 Alert Triage Complete for {{ .source.data.alert.alertname }}
      
      **Recommendations:**
      {{ .workflow.outputs.recommendations }}
      
      **Severity Assessment:** {{ .workflow.outputs.severity-assessment }}
      
      **Context:**
      - Source: {{ .source.name }}
      - Workflow: {{ .workflow.name }}
      - Duration: {{ .workflow.duration }}
---
apiVersion: punchingfist.io/v1alpha1
kind: Sink
metadata:
  name: alertmanager-update
spec:
  type: alertmanager
  config:
    endpoint: "http://alertmanager:9093"
    action: "annotate"
    template: |
      ai_triage: "{{ .workflow.outputs.recommendations }}"
      ai_severity: "{{ .workflow.outputs.severity-assessment }}"
      ai_timestamp: "{{ .workflow.completedAt }}"
```

### OperatorConfig
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: OperatorConfig
metadata:
  name: punchingfist-config
spec:
  database:
    type: "postgres"
    connectionString: "postgresql://user:pass@postgres:5432/punchingfist"
  chat:
    slack:
      botToken: "xoxb-secret"
      channelId: "C1234567890"
  llm:
    defaultProvider: "local"
    providers:
      local:
        endpoint: "http://llm-service:8080"
        model: "llama-3.1-70b"
      claude:
        apiKey: "sk-secret"
        model: "claude-3-sonnet-20240229"
  runbooks:
    repository: "https://github.com/company/runbooks"
    syncInterval: "1h"
```

#### 5. **Chat Interface**

### 1. **Source Event Ingestion**
- Source handlers receive events (webhooks, chat commands, schedules)
- Event data is validated and filtered based on Source configuration
- Matching events trigger the specified Workflow
- Source context is passed to the Workflow execution

### 2. **Workflow Execution**
- Workflow engine creates execution context with Source data
- Steps execute sequentially with access to previous step outputs
- Agent tasks perform LLM-powered reasoning loops within steps
- Approval gates pause execution for human interaction when configured

### 3. **Sink Output Processing**
- Workflow outputs are processed through configured Sinks
- Each Sink formats data according to its template and destination
- Multiple Sinks can run in parallel for the same Workflow
- Sink execution status is tracked for observability

### 4. **State Management**
- Complete execution history stored in database
- Cross-workflow context maintained for learning
- Failed executions can be retried with exponential backoff
- Metrics and traces captured for all pipeline stages

## LLM Agent Investigation Examples

### Scenario 1: Pod Crash Loop Investigation
```
Alert: PodCrashLooping - my-app-pod-xyz
↓
Agent starts investigation:
1. kubectl describe pod my-app-pod-xyz
   → "Exit code 137 (SIGKILL), memory limit exceeded"
2. kubectl logs my-app-pod-xyz --previous
   → "OutOfMemoryError: Java heap space"
3. promql query: container_memory_usage_bytes{pod="my-app-pod-xyz"}
   → Memory usage spiking to 512MB (pod limit)
4. Check recent deployment history
   → New version deployed 2 hours ago
↓
Agent conclusion: "Memory limit too low for new version. Recommend increasing limit to 1GB"
↓
Output: Slack notification with full investigation + suggested kubectl patch command
```

### Scenario 2: API Response Time Alert
```
Alert: HighAPIResponseTime - payment-service
↓
Agent investigation:
1. promql: rate(http_request_duration_seconds[5m])
   → 95th percentile response time increased from 100ms to 2s
2. kubectl logs -l app=payment-service --tail=200
   → "Connection timeout to database"
3. kubectl get pods -l app=postgres
   → Database pod shows high CPU usage
4. promql: rate(postgres_queries_total[5m])
   → Query rate normal, but duration increased
↓
Agent reasoning: "Database performance degraded, likely needs optimization or scaling"
↓
Auto-action: Scale database pod, monitor for improvement
↓
Output: "Scaled database pod from 1 to 2 replicas, monitoring response times"
```

### Scenario 3: Network Connectivity Issue
```
Alert: ServiceUnavailable - user-auth-service
↓
Agent investigation:
1. kubectl get svc user-auth-service
   → Service endpoints exist
2. kubectl get pods -l app=user-auth
   → All pods running and ready
3. curl http://user-auth-service:8080/health
   → Connection timeout
4. kubectl exec -it user-auth-pod -- netstat -ln
   → Service listening on port 8080
5. kubectl describe svc user-auth-service
   → Service targeting wrong port (8080 vs 3000)
↓
Agent fix: kubectl patch svc user-auth-service -p '{"spec":{"ports":[{"port":8080,"targetPort":3000}]}}'
↓
Verification: curl http://user-auth-service:8080/health → 200 OK
↓
Output: "Fixed service port mapping, service now accessible"
```

### Scenario 4: Complex Multi-Service Issue
```
Alert: MultipleServicesDegraded
↓
Agent investigation process:
1. Identify affected services from alert labels
2. For each service:
   - Check pod status and logs
   - Query relevant metrics (CPU, memory, request rates)
   - Test inter-service connectivity
3. Look for common patterns:
   - Shared database performance
   - Network policy changes
   - Recent deployments
4. Cross-reference timing of issues
↓
Agent discovers: All services share Redis cache, which is experiencing high memory usage
↓
Recommendation: "Redis memory usage at 95%, recommend increasing memory limit or implementing cache eviction"
```

## Agent Reasoning Capabilities

### **Investigation Methodology**
The agent follows systematic debugging approaches:
- **Top-down**: Start with high-level metrics, drill down to specifics
- **Correlation**: Look for timing patterns across related components  
- **Historical**: Compare current state with recent baselines
- **Dependency mapping**: Understand service relationships

### **Tool Usage Patterns**
- **kubectl describe** → Initial resource status
- **kubectl logs** → Application-level errors  
- **promql queries** → Performance metrics and trends
- **Network tools** → Connectivity verification
- **Custom scripts** → Complex analysis workflows

### **Decision Making**
- **Safety-first**: Never execute destructive commands without approval
- **Confidence scoring**: Express uncertainty when evidence is ambiguous
- **Context preservation**: Maintain investigation state across iterations
- **Learning**: Remember successful investigation patterns

### Supported Source Types

#### **webhook**
```yaml
type: webhook
config:
  path: "/webhook/alerts"
  filters:
    severity: ["critical"]
  authentication:
    type: "bearer"
    secretRef: "webhook-secret"
```

#### **chat**
```yaml
type: chat
config:
  platform: "slack"
  trigger: "mention"  # or "command"
  channel: "#ops"
  command: "debug"  # for command triggers
```

#### **schedule**
```yaml
type: schedule
config:
  cron: "0 2 * * *"  # Daily at 2 AM
  timezone: "UTC"
```

#### **api**
```yaml
type: api
config:
  endpoint: "/api/v1/trigger"
  method: "POST"
  authentication:
    type: "apikey"
```

#### **kubernetes**
```yaml
type: kubernetes
config:
  resource: "pods"
  event: "created"
  labelSelector: "app=critical-service"
```

### Supported Sink Types

#### **slack**
```yaml
type: slack
config:
  channel: "#alerts"
  messageType: "thread"  # or "message"
  mentionUsers: ["@oncall"]
```

#### **alertmanager**
```yaml
type: alertmanager
config:
  action: "resolve"  # or "annotate", "silence"
  endpoint: "http://alertmanager:9093"
```

#### **prometheus**
```yaml
type: prometheus
config:
  pushgateway: "http://pushgateway:9091"
  job: "punchingfist-results"
```

#### **jira**
```yaml
type: jira
config:
  project: "OPS"
  issueType: "Incident"
  endpoint: "https://company.atlassian.net"
```

#### **pagerduty**
```yaml
type: pagerduty
config:
  routingKey: "service-key"
  action: "trigger"  # or "resolve"
```

#### **workflow**
```yaml
type: workflow
config:
  workflowName: "escalation-workflow"
  triggerCondition: "severity == 'critical'"
```

## Rig Integration Architecture

### **Core Abstraction Layers**

#### **1. Provider Management**
```rust
// crates/operator/src/agent/llm_provider.rs
use rig::providers::{self, Provider};

pub enum LLMProvider {
    Local(providers::local::Client),
    Anthropic(providers::anthropic::Client),
    OpenAI(providers::openai::Client),
    Together(providers::together::Client),
}

impl LLMProvider {
    pub fn from_config(config: &LLMConfig) -> Result<Self> {
        match config.provider.as_str() {
            "local" => Ok(Self::Local(
                providers::local::Client::new(&config.endpoint)
            )),
            "anthropic" => Ok(Self::Anthropic(
                providers::anthropic::Client::from_env()
            )),
            "openai" => Ok(Self::OpenAI(
                providers::openai::Client::from_env()
            )),
            "together" => Ok(Self::Together(
                providers::together::Client::from_env()
            )),
            _ => Err(Error::Config(format!("Unsupported provider: {}", config.provider)))
        }
    }
}
```

#### **2. Tool Implementation**
```rust
// crates/operator/src/agent/tools.rs
use rig::tool::{Tool, ToolDescription, ToolResult};

#[derive(Tool)]
#[tool(description = "Execute kubectl commands for Kubernetes inspection")]
pub struct KubectlTool {
    client: kube::Client,
    allowed_verbs: Vec<String>,
}

impl KubectlTool {
    async fn execute(&self, command: &str) -> ToolResult {
        // Parse and validate kubectl command
        // Execute via Kubernetes API
        // Return formatted result
    }
}

#[derive(Tool)]
#[tool(description = "Query Prometheus metrics using PromQL")]
pub struct PromQLTool {
    prometheus_url: String,
    auth_token: Option<String>,
}

#[derive(Tool)]
#[tool(description = "Perform HTTP requests for health checks")]
pub struct CurlTool {
    allowed_domains: Vec<String>,
}
```

#### **3. Agent Runtime**
```rust
// crates/operator/src/agent/runtime.rs
use rig::{agent::Agent, completion::Prompt};

pub struct AgentRuntime {
    provider: LLMProvider,
    tools: Vec<Box<dyn Tool>>,
    max_iterations: u32,
}

impl AgentRuntime {
    pub async fn investigate(&self, goal: &str, context: &WorkflowContext) -> Result<AgentResult> {
        // Build agent with tools
        let agent = self.provider
            .agent("investigation")
            .preamble(INVESTIGATION_PREAMBLE)
            .tools(&self.tools)
            .max_iterations(self.max_iterations)
            .build();

        // Create investigation prompt with context
        let prompt = format!(
            "Goal: {}\n\nContext:\n{}",
            goal,
            serde_json::to_string_pretty(&context)?
        );

        // Execute agent reasoning loop
        let response = agent.prompt(&prompt).await?;
        
        // Parse and structure results
        Ok(AgentResult {
            findings: response.content,
            confidence: response.metadata.confidence,
            actions_taken: response.tool_calls,
            recommendations: self.extract_recommendations(&response),
        })
    }
}
```

### **Workflow Integration**

The Rig-powered agent integrates seamlessly with the workflow engine:

```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Workflow
spec:
  runtime:
    llmConfig:
      provider: "local"  # or "anthropic", "openai", "together"
      endpoint: "http://llm-service:8080"  # for local provider
      model: "llama-3.1-70b"
      temperature: 0.7
      maxTokens: 4096
  
  steps:
    - name: "investigate-alert"
      type: "agent"
      goal: "Investigate the alert and determine root cause"
      tools:
        - kubectl
        - promql
        - curl
      rigConfig:
        # Rig-specific configuration
        retryPolicy:
          maxAttempts: 3
          backoff: "exponential"
        safety:
          requireApproval: ["delete", "patch", "scale"]
```

### **Safety & Governance**

Rig integration includes built-in safety features:

1. **Tool Sandboxing**: Each tool validates and sanitizes inputs
2. **Approval Gates**: Destructive operations require human confirmation
3. **Audit Logging**: All LLM interactions and tool executions logged
4. **Token Limits**: Prevent runaway costs with configurable limits
5. **Rate Limiting**: Provider-specific rate limit handling

### **Performance Optimizations**

1. **Connection Pooling**: Reuse LLM provider connections
2. **Response Caching**: Cache similar investigations (configurable TTL)
3. **Streaming Support**: Process LLM responses as they arrive
4. **Parallel Tool Execution**: Run independent tools concurrently

## Safety & Reliability

### Circuit Breakers
- **Max Failures**: Configurable per task (default: 3)
- **Backoff Strategy**: Exponential backoff for retries
- **Global Limits**: Rate limiting on task creation

### Approval Gates
- **Configuration-driven**: Defined in WorkflowTemplate
- **Conditions**: Based on alert severity, affected resources, etc.
- **Chat Integration**: Slack-based approval workflow

### Validation Layer
- **Pre-execution**: Validate commands against allowed patterns
- **RBAC Integration**: Respect service account permissions
- **Dry-run Support**: Test mode for workflow validation

## Observability

### Metrics (Prometheus)
```rust
// Example metrics
counter!("punchingfist_alerts_processed_total", "source" => alert_source);
histogram!("punchingfist_task_duration_seconds", duration.as_secs_f64());
gauge!("punchingfist_active_tasks", active_tasks as f64);
```

### Logging & Tracing
- **Tracing Crate**: Structured logging with correlation IDs
- **Log Levels**: DEBUG, INFO, WARN, ERROR
- **Trace Propagation**: End-to-end request tracing

### Key Metrics
- Alert processing latency
- Task success/failure rates
- LLM API usage and costs
- Chat response times
- Resource utilization

## Security & Secret Management

### Secret Types & Sources

#### **Operator-Level Secrets**
```yaml
# Stored as Kubernetes Secrets, mounted into operator pod
apiVersion: v1
kind: Secret
metadata:
  name: punchingfist-config
type: Opaque
data:
  llm-api-key: <base64>          # Claude/OpenAI API keys
  slack-bot-token: <base64>      # Chat integration
  database-url: <base64>         # Postgres connection string
  prometheus-token: <base64>     # Metrics system access
```

#### **External Secret Manager Integration**
```yaml
# For enterprise environments
apiVersion: punchingfist.io/v1alpha1
kind: OperatorConfig
spec:
  secretManager:
    type: "vault"  # or "aws-secrets", "azure-keyvault"
    endpoint: "https://vault.company.com"
    authMethod: "kubernetes"
    secretPath: "secret/punchingfist/"
    
  # Alternative: direct K8s secrets
  secrets:
    llmApiKey:
      secretRef: "llm-credentials"
      key: "api-key"
    slackToken:
      secretRef: "chat-credentials" 
      key: "bot-token"
```

### Agent Runtime Security Model

#### **ServiceAccount-Based RBAC**
```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: punchingfist-agent
  namespace: punchingfist-system
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: punchingfist-investigator
rules:
  # Read-only cluster inspection
  - apiGroups: [""]
    resources: ["pods", "services", "nodes", "events"]
    verbs: ["get", "list", "describe"]
  - apiGroups: [""]
    resources: ["pods/log"]
    verbs: ["get"]
  - apiGroups: ["apps"]
    resources: ["deployments", "replicasets"]
    verbs: ["get", "list"]
  
  # Limited write permissions (with approval gates)
  - apiGroups: [""]
    resources: ["pods"]
    verbs: ["delete"]  # For pod restart (requires approval)
  - apiGroups: ["apps"] 
    resources: ["deployments/scale"]
    verbs: ["patch"]   # For scaling (requires approval)

# Separate role for high-privilege operations
---
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: punchingfist-remediator
rules:
  - apiGroups: [""]
    resources: ["pods", "services", "configmaps"]
    verbs: ["*"]
  - apiGroups: ["apps"]
    resources: ["deployments"]
    verbs: ["*"]
```

#### **Multi-Level Permission Model**
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Workflow
metadata:
  name: tiered-investigation
spec:
  steps:
    - name: "safe-investigation"
      type: "agent"
      serviceAccount: "punchingfist-investigator"  # Read-only
      goal: "Investigate the alert using read-only operations"
      
    - name: "remediation"
      type: "agent" 
      serviceAccount: "punchingfist-remediator"    # Write access
      approvalRequired: true                       # Human gate
      goal: "Execute the remediation plan"
```

### Secret Injection into Agent Runtime

#### **Environment-Based Injection**
```yaml
apiVersion: v1
kind: Pod
spec:
  serviceAccount: punchingfist-agent
  containers:
  - name: agent-runtime
    image: punchingfist/runtime:v1.0.0
    env:
    - name: PROMETHEUS_URL
      value: "http://prometheus:9090"
    - name: PROMETHEUS_TOKEN
      valueFrom:
        secretKeyRef:
          name: monitoring-credentials
          key: prometheus-token
    - name: LLM_API_KEY
      valueFrom:
        secretKeyRef:
          name: llm-credentials
          key: api-key
    # ServiceAccount token auto-mounted at standard location
    volumeMounts:
    - name: kube-api-access
      mountPath: /var/run/secrets/kubernetes.io/serviceaccount
      readOnly: true
```

#### **Dynamic Secret Resolution**
```rust
// In the agent runtime
pub struct SecretManager {
    k8s_client: Client,
    vault_client: Option<VaultClient>,
}

impl SecretManager {
    async fn get_secret(&self, secret_ref: &str) -> Result<String> {
        match secret_ref {
            s if s.starts_with("k8s://") => {
                // Fetch from Kubernetes Secret
                self.get_k8s_secret(s).await
            }
            s if s.starts_with("vault://") => {
                // Fetch from Vault
                self.get_vault_secret(s).await
            }
            s if s.starts_with("env://") => {
                // Get from environment variable
                std::env::var(s.strip_prefix("env://").unwrap())
                    .map_err(|e| Error::EnvVar(e))
            }
            _ => Err(Error::UnsupportedSecretRef(secret_ref.to_string()))
        }
    }
}
```

### Tool-Specific Secret Handling

#### **Prometheus Queries with Authentication**
```bash
# Custom promql tool that handles auth transparently
#!/bin/bash
# /usr/local/bin/promql

QUERY="$1"
PROMETHEUS_URL="${PROMETHEUS_URL:-http://prometheus:9090}"

# Use token from secret if available
if [ -n "$PROMETHEUS_TOKEN" ]; then
    AUTH_HEADER="Authorization: Bearer $PROMETHEUS_TOKEN"
else
    AUTH_HEADER=""
fi

curl -s -H "$AUTH_HEADER" \
    "${PROMETHEUS_URL}/api/v1/query" \
    --data-urlencode "query=${QUERY}" \
    | jq '.data.result'
```

#### **kubectl with Different Contexts**
```bash
# Agent can use different kubeconfigs for different operations
export KUBECONFIG=/var/run/secrets/kubernetes.io/serviceaccount/kubeconfig

# For cross-cluster investigations (if configured)
if [ -n "$REMOTE_CLUSTER_CONFIG" ]; then
    export KUBECONFIG="$REMOTE_CLUSTER_CONFIG"
fi

kubectl "$@"
```

### Security Best Practices

#### **Principle of Least Privilege**
- **Investigation Phase**: Read-only access to most resources
- **Remediation Phase**: Write access only with human approval
- **Per-Workflow RBAC**: Different workflows can have different permission levels
- **Namespace Scoping**: Agent access can be limited to specific namespaces

#### **Secret Rotation Support**
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: OperatorConfig
spec:
  secretRotation:
    enabled: true
    checkInterval: "1h"
    gracePeriod: "300s"  # Allow in-flight operations to complete
```

#### **Audit Trail**
```rust
// All secret access is logged
info!(
    secret_ref = %secret_ref,
    workflow_id = %workflow_id,
    agent_step = %step_name,
    "Secret accessed for agent operation"
);
```

#### **Network Security**
- **TLS Everywhere**: All external API calls use TLS
- **Certificate Validation**: Verify certificates for external systems
- **Network Policies**: Restrict agent pod network access
- **Secret Transit**: Secrets never logged or exposed in outputs

### Enterprise Integration Examples

#### **AWS Secrets Manager**
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: OperatorConfig
spec:
  secretManager:
    type: "aws-secrets"
    region: "us-west-2"
    authMethod: "iam-role"  # Using IRSA
    secretPrefix: "punchingfist/"
```

#### **HashiCorp Vault**
```yaml
apiVersion: punchingfist.io/v1alpha1  
kind: OperatorConfig
spec:
  secretManager:
    type: "vault"
    endpoint: "https://vault.company.com"
    authMethod: "kubernetes"
    role: "punchingfist-agent"
    secretPath: "secret/data/punchingfist"
```

#### **Azure Key Vault**
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: OperatorConfig
spec:
  secretManager:
    type: "azure-keyvault"
    vaultUrl: "https://company-vault.vault.azure.net/"
    authMethod: "managed-identity"
    clientId: "12345678-1234-1234-1234-123456789012"
```

## Implementation Phases

### Phase 1: MVP (Smart Alert Middleware)
- [ ] Core Source/Workflow/Sink Custom Resources
- [ ] Webhook Source handler for AlertManager integration
- [ ] Alerts database table and basic lifecycle tracking
- [ ] Basic Workflow engine with CLI step execution and agent tasks
- [ ] Slack Sink for enriched notifications
- [ ] Simple auto-resolution for low-risk scenarios
- [ ] SQLite state storage
- [ ] Alert deduplication and fingerprinting

### Phase 2: Incident Management Core
- [ ] Incidents table and alert correlation
- [ ] Intelligent escalation decision matrix
- [ ] PagerDuty/Opsgenie Sink integration
- [ ] Enhanced AI triage with confidence scoring
- [ ] PostgreSQL support with performance optimization
- [ ] Multiple LLM provider support
- [ ] Chat Source for manual incident commands
- [ ] Basic learning from resolution patterns

### Phase 3: Advanced IRM Features
- [ ] ML-powered pattern recognition and prediction
- [ ] Advanced incident correlation and root cause analysis
- [ ] Comprehensive metrics and dashboards
- [ ] Multi-team/multi-environment support
- [ ] Advanced approval workflows and delegation
- [ ] Integration with ticketing systems (JIRA, ServiceNow)
- [ ] Runbook automation and knowledge base building

### Phase 4: Enterprise & Scale
- [ ] Multi-cluster federation
- [ ] Advanced security and compliance features
- [ ] Custom ML model training on organizational data
- [ ] API for external integrations
- [ ] Advanced reporting and analytics
- [ ] Predictive incident prevention

## Configuration Example

### Complete Alert Triage Setup

#### 1. AlertManager Integration
```yaml
# alertmanager.yml
route:
  group_by: ['alertname']
  routes:
  - match:
      severity: critical
    receiver: 'punchingfist-webhook'

receivers:
- name: 'punchingfist-webhook'
  webhook_configs:
  - url: 'http://punchingfist-operator:8080/webhook/alerts'
    send_resolved: true
```

#### 2. Source Configuration
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Source
metadata:
  name: critical-alerts
spec:
  type: webhook
  config:
    path: "/webhook/alerts"
    filters:
      severity: ["critical"]
  triggerWorkflow: "alert-triage"
```

#### 3. Workflow Definition
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Workflow
metadata:
  name: alert-triage
spec:
  runtime:
    image: "punchingfist/runtime:v1.0.0"
    llmConfig:
      provider: "local"
      endpoint: "http://llm-service:8080"
  steps:
    - name: "initial-triage"
      type: "agent"
      goal: "Analyze the alert and gather initial diagnostic information"
      tools: ["kubectl", "curl"]
      context: "{{ .source.data }}"
  sinks: ["slack-notification", "alert-annotation"]
```

#### 4. Sink Outputs
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Sink
metadata:
  name: slack-notification
spec:
  type: slack
  config:
    channel: "#ops-alerts"
    template: |
      🤖 **Alert Triage Results**
      Alert: {{ .source.data.alert.alertname }}
      {{ .workflow.outputs.initial-triage }}
---
apiVersion: punchingfist.io/v1alpha1
kind: Sink
metadata:
  name: alert-annotation
spec:
  type: alertmanager
  config:
    action: "annotate"
    template: |
      ai_analysis: "{{ .workflow.outputs.initial-triage }}"
```

## Dependencies

### Rust Crates
- **kube**: Kubernetes API interaction
- **tokio**: Async runtime
- **serde**: Serialization/deserialization
- **sqlx**: Database abstraction
- **reqwest**: HTTP client
- **rig**: LLM integration and agent framework
- **prometheus**: Metrics collection
- **tracing**: Logging and tracing
- **slack-api**: Chat integration

### External Dependencies
- Kubernetes cluster (1.20+)
- PostgreSQL or SQLite
- Container runtime (Docker/Containerd)
- LLM endpoint (local or cloud)
  - Local: Ollama, vLLM, or compatible API
  - Cloud: Anthropic, OpenAI, Together.ai

## Success Metrics

### Primary KPIs (Incident Response Management)
- **Alert Noise Reduction**: Percentage of alerts auto-resolved without human intervention
- **Mean Time to Resolution (MTTR)**: Average time from alert received to resolution
- **Mean Time to Triage (MTTT)**: Average time from alert received to AI analysis completion
- **Escalation Accuracy**: Percentage of escalated alerts that required human intervention
- **False Positive Reduction**: Decrease in alerts that resolve themselves after escalation
- **Context Quality Score**: Human rating of AI-provided context and recommendations

### Secondary Metrics
- **Auto-Resolution Success Rate**: Percentage of attempted auto-resolutions that succeeded
- **Incident Correlation Accuracy**: How well AI groups related alerts into incidents
- **On-Call Burden Reduction**: Decrease in out-of-hours human interventions
- **Cost Savings**: Reduced operational overhead from improved incident response

### Operational Metrics
- Workflow execution latency
- LLM API cost efficiency  
- Database query performance
- System resource utilization
- Alert processing throughput

## Future Considerations

### Potential Enhancements
- **Multi-cluster Support**: Federated operator deployment
- **Advanced Learning**: ML-based pattern recognition
- **Custom Tool Integration**: Plugin architecture for specialized tools
- **Predictive Maintenance**: Proactive issue detection
- **Integration Ecosystem**: Support for additional monitoring/chat platforms

---

*This design document represents the initial architecture for Punching Fist Operator. It will evolve based on implementation feedback and operational requirements.*