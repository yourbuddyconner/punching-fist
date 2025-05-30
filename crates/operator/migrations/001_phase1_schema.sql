-- Punching Fist Operator - Phase 1 Schema
-- This creates the complete database schema for the Source → Workflow → Sink architecture

-- Alerts table with full lifecycle tracking
CREATE TABLE IF NOT EXISTS alerts (
    id UUID PRIMARY KEY,
    external_id VARCHAR(255),
    fingerprint VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL,
    severity VARCHAR(50) NOT NULL,
    alert_name VARCHAR(255) NOT NULL,
    name VARCHAR(255) NOT NULL, -- Kept for backward compatibility
    summary TEXT,
    description TEXT,
    labels TEXT NOT NULL, -- JSON stored as text
    annotations TEXT NOT NULL, -- JSON stored as text
    source_id UUID,
    workflow_id UUID,
    
    -- AI Analysis
    ai_analysis TEXT, -- JSON stored as text
    ai_confidence REAL,
    auto_resolved BOOLEAN DEFAULT FALSE,
    
    -- Timing
    starts_at TIMESTAMP NOT NULL,
    ends_at TIMESTAMP,
    received_at TIMESTAMP NOT NULL,
    triage_started_at TIMESTAMP,
    triage_completed_at TIMESTAMP,
    resolved_at TIMESTAMP,
    
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

-- Workflows table for execution tracking
CREATE TABLE IF NOT EXISTS workflows (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    namespace VARCHAR(255) NOT NULL,
    trigger_source VARCHAR(255),
    status VARCHAR(50) NOT NULL,
    
    -- Execution details
    steps_completed INTEGER DEFAULT 0,
    total_steps INTEGER NOT NULL,
    current_step VARCHAR(255),
    
    -- Context and results (JSON stored as text)
    input_context TEXT,
    outputs TEXT,
    error TEXT,
    
    -- Timing
    started_at TIMESTAMP NOT NULL,
    completed_at TIMESTAMP,
    
    created_at TIMESTAMP NOT NULL
);

-- Source events table
CREATE TABLE IF NOT EXISTS source_events (
    id UUID PRIMARY KEY,
    source_name VARCHAR(255) NOT NULL,
    source_type VARCHAR(50) NOT NULL,
    event_data TEXT NOT NULL, -- JSON stored as text
    workflow_triggered VARCHAR(255),
    
    received_at TIMESTAMP NOT NULL
);

-- Workflow steps table
CREATE TABLE IF NOT EXISTS workflow_steps (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    name VARCHAR(255) NOT NULL,
    step_type VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    
    -- Step configuration (JSON stored as text)
    config TEXT,
    
    -- Execution details
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    result TEXT, -- JSON stored as text
    error TEXT,
    
    created_at TIMESTAMP NOT NULL
);

-- Sink outputs table
CREATE TABLE IF NOT EXISTS sink_outputs (
    id UUID PRIMARY KEY,
    workflow_id UUID NOT NULL REFERENCES workflows(id),
    sink_name VARCHAR(255) NOT NULL,
    sink_type VARCHAR(50) NOT NULL,
    
    -- Output details
    payload TEXT, -- JSON stored as text
    status VARCHAR(50) NOT NULL,
    error TEXT,
    
    sent_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL
);

-- Custom resources table (for storing CRD instances)
CREATE TABLE IF NOT EXISTS custom_resources (
    id UUID PRIMARY KEY,
    api_version VARCHAR(255) NOT NULL,
    kind VARCHAR(50) NOT NULL,
    name VARCHAR(255) NOT NULL,
    namespace VARCHAR(255) NOT NULL,
    spec TEXT NOT NULL, -- JSON stored as text
    status TEXT, -- JSON stored as text
    
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    
    UNIQUE(kind, namespace, name)
);

-- Legacy tasks table (kept for compatibility, will be removed in Phase 2)
CREATE TABLE IF NOT EXISTS tasks (
    id UUID PRIMARY KEY,
    alert_id UUID NOT NULL REFERENCES alerts(id),
    prompt TEXT NOT NULL,
    model VARCHAR(255) NOT NULL,
    status INTEGER NOT NULL,
    max_retries INTEGER NOT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    timeout INTEGER NOT NULL,
    resources TEXT NOT NULL, -- JSON stored as text
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    error TEXT
);

-- Create all indexes
CREATE INDEX IF NOT EXISTS idx_alerts_fingerprint ON alerts(fingerprint);
CREATE INDEX IF NOT EXISTS idx_alerts_external_id ON alerts(external_id);
CREATE INDEX IF NOT EXISTS idx_alerts_status ON alerts(status);
CREATE INDEX IF NOT EXISTS idx_alerts_severity ON alerts(severity);
CREATE INDEX IF NOT EXISTS idx_alerts_received_at ON alerts(received_at);
CREATE INDEX IF NOT EXISTS idx_alerts_source_id ON alerts(source_id);
CREATE INDEX IF NOT EXISTS idx_alerts_workflow_id ON alerts(workflow_id);
CREATE INDEX IF NOT EXISTS idx_alerts_created_at ON alerts(created_at);

CREATE INDEX IF NOT EXISTS idx_workflows_status ON workflows(status);
CREATE INDEX IF NOT EXISTS idx_workflows_started_at ON workflows(started_at);
CREATE INDEX IF NOT EXISTS idx_workflows_namespace ON workflows(namespace);

CREATE INDEX IF NOT EXISTS idx_source_events_source_name ON source_events(source_name);
CREATE INDEX IF NOT EXISTS idx_source_events_received_at ON source_events(received_at);

CREATE INDEX IF NOT EXISTS idx_workflow_steps_workflow_id ON workflow_steps(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_steps_status ON workflow_steps(status);

CREATE INDEX IF NOT EXISTS idx_sink_outputs_workflow_id ON sink_outputs(workflow_id);
CREATE INDEX IF NOT EXISTS idx_sink_outputs_sink_name ON sink_outputs(sink_name);

CREATE INDEX IF NOT EXISTS idx_custom_resources_kind ON custom_resources(kind);
CREATE INDEX IF NOT EXISTS idx_custom_resources_namespace ON custom_resources(namespace);

CREATE INDEX IF NOT EXISTS idx_tasks_created_at ON tasks(created_at);
CREATE INDEX IF NOT EXISTS idx_tasks_alert_id ON tasks(alert_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status); 