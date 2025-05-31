# Phase 1 Implementation Plan - Punching Fist Operator

## Overview
Phase 1 transforms the operator from OpenHands integration to the new Source → Workflow → Sink architecture, establishing the foundation for an intelligent incident response system.

## Current State vs Target State

### Current State
- OpenHands-based task execution
- Basic alert ingestion
- Kubernetes Job-based execution
- Simple webhook server

### Target State (Phase 1)
- Complete Source/Workflow/Sink event-driven architecture
- AlertManager webhook integration with filtering
- LLM agent-powered workflow execution
- Slack notifications with enriched context
- SQLite-based state tracking
- Alert deduplication and lifecycle management

## Implementation Timeline (6 weeks)

### Week 1: Core CRD Implementation ✅ (Started)
**Goal**: Define and implement Custom Resource Definitions

#### Tasks Completed:
- [x] Create CRD module structure
- [x] Define Source CRD with webhook, chat, schedule types
- [x] Define Workflow CRD with CLI and agent step types
- [x] Define Sink CRD with Slack and AlertManager outputs
- [x] Create common types for event context

#### Next Tasks:
- [ ] Add CRD validation and defaulting webhooks
- [ ] Generate CRD YAML manifests for installation
- [ ] Create example CRs for testing
- [ ] Update Cargo.toml with required dependencies

### Week 2: Database Schema & Store Layer
**Goal**: Implement new database schema for alert lifecycle tracking

#### Tasks:
- [ ] Create new migration for Phase 1 schema:
  ```sql
  -- Alerts table (enhanced from current)
  CREATE TABLE alerts (
      id UUID PRIMARY KEY,
      external_id VARCHAR(255) UNIQUE,
      fingerprint VARCHAR(255) NOT NULL,
      status VARCHAR(50) NOT NULL,
      severity VARCHAR(20) NOT NULL,
      alert_name VARCHAR(255) NOT NULL,
      summary TEXT,
      description TEXT,
      labels JSONB,
      annotations JSONB,
      source_id UUID,
      workflow_id UUID,
      
      -- AI Analysis
      ai_analysis JSONB,
      ai_confidence FLOAT,
      auto_resolved BOOLEAN DEFAULT FALSE,
      
      -- Timing
      received_at TIMESTAMP NOT NULL,
      triage_started_at TIMESTAMP,
      triage_completed_at TIMESTAMP,
      resolved_at TIMESTAMP,
      
      created_at TIMESTAMP DEFAULT NOW(),
      updated_at TIMESTAMP DEFAULT NOW()
  );
  
  -- Workflows table
  CREATE TABLE workflows (
      id UUID PRIMARY KEY,
      name VARCHAR(255) NOT NULL,
      namespace VARCHAR(255) NOT NULL,
      trigger_source VARCHAR(255),
      status VARCHAR(50) NOT NULL,
      
      -- Execution details
      steps_completed INTEGER DEFAULT 0,
      total_steps INTEGER NOT NULL,
      current_step VARCHAR(255),
      
      -- Context and results
      input_context JSONB,
      outputs JSONB,
      error TEXT,
      
      -- Timing
      started_at TIMESTAMP NOT NULL,
      completed_at TIMESTAMP,
      
      created_at TIMESTAMP DEFAULT NOW()
  );
  
  -- Source events table
  CREATE TABLE source_events (
      id UUID PRIMARY KEY,
      source_name VARCHAR(255) NOT NULL,
      source_type VARCHAR(50) NOT NULL,
      event_data JSONB NOT NULL,
      workflow_triggered VARCHAR(255),
      
      received_at TIMESTAMP DEFAULT NOW()
  );
  ```
- [ ] Implement new Store trait for Phase 1 entities
- [ ] Create SQLite and PostgreSQL implementations
- [ ] Add alert deduplication logic using fingerprints
- [ ] Implement alert lifecycle state transitions

### Week 3: Source Handlers & Webhook Server
**Goal**: Implement webhook source for AlertManager integration

#### Tasks:
- [ ] Refactor current webhook server for new architecture
- [ ] Implement Source controller that watches Source CRs
- [ ] Create webhook handler for AlertManager format:
  ```rust
  // Parse AlertManager webhook payload
  // Apply Source filters (severity, alertname)
  // Generate fingerprint for deduplication
  // Store in database
  // Trigger configured workflow
  ```
- [ ] Implement alert filtering based on Source config
- [ ] Add authentication support (bearer token)
- [ ] Create source event routing to workflow engine
- [ ] Add metrics for webhook processing

### Week 4: Workflow Engine Core
**Goal**: Build the workflow execution engine

#### Tasks:
- [ ] Create Workflow controller that watches Workflow CRs
- [ ] Implement workflow execution state machine:
  ```
  Pending → Running → Succeeded/Failed
  ```
- [ ] Build step executor for different step types:
  - CLI step: Execute commands in container
  - Agent step: LLM reasoning loop
  - Conditional step: Evaluate conditions
- [ ] Implement context passing between steps
- [ ] Add timeout and retry logic
- [ ] Create workflow status updates
- [ ] Implement output collection from steps

### Week 5: LLM Agent Runtime
**Goal**: Implement agent-based investigation capabilities using Rig

#### Tasks:
- [ ] Add Rig dependency to Cargo.toml:
  ```toml
  [dependencies]
  rig-core = "0.1"
  ```
- [ ] Create LLM provider abstraction using Rig:
  ```rust
  // Support for local, Anthropic, OpenAI, Together providers
  // Configuration-driven provider selection
  // Connection pooling and retry logic
  ```
- [ ] Implement tool definitions with Rig:
  - KubectlTool: Kubernetes API-based command execution
  - PromQLTool: Prometheus metric queries
  - CurlTool: HTTP health checks
  - CustomScriptTool: Shell script execution
- [ ] Build agent runtime with Rig:
  ```rust
  // Agent creation with configured tools
  // Investigation loop with max iterations
  // Structured output parsing
  // Safety checks and approval gates
  ```
- [ ] Create agent step executor:
  - Parse goal and tools from workflow step
  - Initialize Rig agent with context
  - Execute reasoning loop
  - Collect and structure outputs
- [ ] Implement safety boundaries:
  - Command validation and sanitization
  - RBAC-aware tool execution
  - Approval gates for destructive operations
- [ ] Add investigation templates:
  - Pod crash investigation
  - High resource usage analysis
  - Network connectivity debugging
  - Service degradation triage
- [ ] Create agent result formatting:
  - Findings summary
  - Confidence scoring
  - Action recommendations
  - Escalation context

#### Implementation Structure:
```
crates/operator/src/agent/
├── mod.rs              # Module exports
├── provider.rs         # Rig provider abstraction
├── runtime.rs          # Agent execution engine
├── tools/              # Tool implementations
│   ├── mod.rs
│   ├── kubectl.rs      # Kubernetes commands
│   ├── promql.rs       # Prometheus queries
│   ├── curl.rs         # HTTP requests
│   └── script.rs       # Custom scripts
├── safety.rs           # Command validation
├── templates.rs        # Investigation templates
└── result.rs           # Output formatting
```

#### Example Agent Configuration:
```yaml
apiVersion: punchingfist.io/v1alpha1
kind: Workflow
spec:
  runtime:
    llmConfig:
      provider: "local"
      endpoint: "http://ollama:11434"
      model: "llama3.1:70b"
  steps:
    - name: "investigate-crash"
      type: "agent"
      goal: "Investigate why pod is crashing"
      tools: ["kubectl", "promql"]
      maxIterations: 10
      timeout: 5m
```

### Week 6: Sink Handlers & Integration
**Goal**: Implement Slack sink and end-to-end testing

#### Tasks:
- [ ] Create Sink controller that watches Sink CRs
- [ ] Implement Slack sink handler:
  - Message formatting with templates
  - Channel posting
  - Thread support for updates
- [ ] Implement AlertManager annotation sink
- [ ] Add sink condition evaluation
- [ ] Create end-to-end test scenarios:
  ```yaml
  # Test: High CPU alert → Investigation → Slack notification
  # Test: Pod crash loop → Auto-resolution → Alert resolved
  # Test: Unknown issue → Enriched context → Escalation
  ```
- [ ] Documentation and deployment guide
- [ ] Performance testing and optimization

## Key Implementation Files Structure

```
crates/operator/src/
├── crd/                    # Custom Resource Definitions
│   ├── mod.rs
│   ├── source.rs          # Source CRD
│   ├── workflow.rs        # Workflow CRD
│   ├── sink.rs           # Sink CRD
│   └── common.rs         # Shared types
├── controllers/           # Kubernetes controllers
│   ├── mod.rs
│   ├── source.rs         # Source controller
│   ├── workflow.rs       # Workflow controller
│   └── sink.rs          # Sink controller
├── sources/              # Source implementations
│   ├── mod.rs
│   ├── webhook.rs       # Webhook handler
│   ├── scheduler.rs     # Cron scheduler
│   └── chat.rs         # Chat bot (future)
├── workflow/            # Workflow engine
│   ├── mod.rs
│   ├── engine.rs       # Core execution engine
│   ├── executor.rs     # Step executor
│   ├── context.rs      # Context management
│   └── state.rs       # State machine
├── agent/              # LLM agent runtime
│   ├── mod.rs
│   ├── runtime.rs     # Agent execution loop
│   ├── llm.rs        # LLM client abstraction
│   ├── tools.rs      # Tool execution
│   └── safety.rs     # Safety checks
├── sinks/             # Sink implementations
│   ├── mod.rs
│   ├── slack.rs      # Slack integration
│   ├── alertmanager.rs # AlertManager updates
│   └── templates.rs  # Template engine
├── store/            # Database layer
│   ├── mod.rs
│   ├── migrations/  # SQL migrations
│   ├── models.rs   # Data models
│   └── queries.rs  # Database queries
└── main.rs         # Application entry point
```

## Testing Strategy

### Unit Tests
- CRD serialization/deserialization
- Alert deduplication logic
- Workflow state transitions
- Template rendering

### Integration Tests
- Source → Workflow triggering
- Workflow → Sink output
- Database operations
- LLM agent execution

### End-to-End Tests
- Complete alert processing pipeline
- Auto-resolution scenarios
- Enrichment and escalation paths

## Success Criteria

1. **Functional Requirements**
   - [ ] Can receive AlertManager webhooks
   - [ ] Can execute LLM-powered investigations
   - [ ] Can send enriched Slack notifications
   - [ ] Can auto-resolve simple issues
   - [ ] Maintains alert lifecycle in database

2. **Performance Requirements**
   - [ ] Webhook processing < 100ms
   - [ ] Workflow startup < 5s
   - [ ] Alert deduplication working correctly
   - [ ] SQLite handling 1000+ alerts

3. **Operational Requirements**
   - [ ] Kubernetes deployment working
   - [ ] Metrics exposed for monitoring
   - [ ] Logs structured and useful
   - [ ] Configuration via CRs

## Risk Mitigation

1. **LLM Integration Complexity**
   - Start with simple prompt templates
   - Build comprehensive tool sandbox
   - Add extensive logging for debugging

2. **Workflow Execution Reliability**
   - Implement proper timeout handling
   - Add retry logic with backoff
   - Store execution state in database

3. **Alert Storm Handling**
   - Implement rate limiting
   - Use fingerprint deduplication
   - Add circuit breakers

## Next Steps After Phase 1

- Phase 2: Incident correlation and management
- Phase 3: Advanced ML patterns and learning
- Phase 4: Multi-cluster and enterprise features 