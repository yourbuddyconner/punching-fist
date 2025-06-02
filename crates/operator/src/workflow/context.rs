use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// The initial input to the workflow
    pub input: Value,
    
    /// Outputs from each completed step
    pub step_outputs: HashMap<String, Value>,
    
    /// Current step being executed
    pub current_step: Option<String>,
    
    /// Additional metadata
    pub metadata: HashMap<String, Value>,
}

impl WorkflowContext {
    pub fn new() -> Self {
        Self {
            input: Value::Object(serde_json::Map::new()),
            step_outputs: HashMap::new(),
            current_step: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_input(input: Value) -> Self {
        Self {
            input,
            step_outputs: HashMap::new(),
            current_step: None,
            metadata: HashMap::new(),
        }
    }

    pub fn set_current_step(&mut self, step_name: &str) {
        self.current_step = Some(step_name.to_string());
    }

    pub fn current_step(&self) -> Option<&str> {
        self.current_step.as_deref()
    }

    pub fn add_step_output(&mut self, step_name: &str, output: Value) {
        self.step_outputs.insert(step_name.to_string(), output);
    }

    pub fn get_step_output(&self, step_name: &str) -> Option<&Value> {
        self.step_outputs.get(step_name)
    }

    pub fn add_metadata(&mut self, key: &str, value: Value) {
        self.metadata.insert(key.to_string(), value);
    }

    pub fn get_metadata(&self, key: &str) -> Option<&Value> {
        self.metadata.get(key)
    }

    /// Convert the context to JSON for storage or transmission
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "input": self.input,
            "step_outputs": self.step_outputs,
            "current_step": self.current_step,
            "metadata": self.metadata,
        })
    }

    /// Create a context from JSON
    pub fn from_json(value: Value) -> Self {
        let empty_map = serde_json::Map::new();
        let obj = value.as_object().unwrap_or(&empty_map);
        
        Self {
            input: obj.get("input").cloned().unwrap_or(Value::Object(serde_json::Map::new())),
            step_outputs: obj.get("step_outputs")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default(),
            current_step: obj.get("current_step")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            metadata: obj.get("metadata")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Get a combined view of all available data for templating
    pub fn get_template_context(&self) -> Value {
        serde_json::json!({
            "input": self.input,
            "outputs": self.step_outputs,
            "metadata": self.metadata,
        })
    }
} 