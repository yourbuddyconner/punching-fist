use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, warn};
use std::collections::HashMap;

use crate::{
    sinks::Sink,
    Result, Error,
    crd::sink::{SinkSpec, SinkConfig, SinkType},
};

pub struct StdoutSink {
    name: String,
    format: String,
    pretty: bool,
    template: Option<String>, // For text output, from SinkConfig.template
}

impl StdoutSink {
    pub fn new(name: String, spec: &SinkSpec) -> Result<Box<dyn Sink>> {
        let config = &spec.config;
        
        let format = config.format.as_ref().unwrap_or(&"json".to_string()).clone();
        let pretty = config.pretty.unwrap_or(false);
        
        // Validate format
        if !["json", "text", "yaml"].contains(&format.as_str()) {
            return Err(Error::Validation(
                format!("Invalid stdout format: {}. Must be one of: json, text, yaml", format)
            ));
        }
        
        // Use the template field from SinkConfig
        let template = config.template.clone();
        
        Ok(Box::new(Self {
            name,
            format,
            pretty,
            template,
        }))
    }
}

#[async_trait]
impl Sink for StdoutSink {
    async fn send(&self, context: Value) -> Result<()> {
        let output = match self.format.as_str() {
            "json" => {
                if self.pretty {
                    serde_json::to_string_pretty(&context)
                        .map_err(|e| Error::Internal(format!("JSON serialization error: {}", e)))?
                } else {
                    serde_json::to_string(&context)
                        .map_err(|e| Error::Internal(format!("JSON serialization error: {}", e)))?
                }
            }
            "yaml" => {
                serde_yaml::to_string(&context)
                    .map_err(|e| Error::Internal(format!("YAML serialization error: {}", e)))?
            }
            "text" => {
                if let Some(tmpl) = &self.template {
                    // Use Tera for template rendering
                    self.render_template(tmpl, &context)?
                } else {
                    // If no template, print the context as JSON
                    serde_json::to_string_pretty(&context)
                        .map_err(|e| Error::Internal(format!("JSON serialization error: {}", e)))?
                }
            }
            _ => unreachable!("Format was validated in new()"),
        };
        
        println!("[{}] {}", self.name, output);
        Ok(())
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

impl StdoutSink {
    fn render_template(&self, template: &str, context: &Value) -> Result<String> {
        crate::template::render_template(template, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    fn create_test_sink_spec(format: Option<&str>, pretty: Option<bool>, template: Option<&str>) -> SinkSpec {
        let mut config = SinkConfig {
            format: format.map(String::from),
            pretty,
            template: template.map(String::from),
            // Initialize all other fields to None/default
            channel: None,
            bot_token: None,
            message_type: None,
            mention_users: vec![],
            endpoint: None,
            action: None,
            pushgateway: None,
            job: None,
            metrics: HashMap::new(),
            project: None,
            issue_type: None,
            credentials_secret: None,
            routing_key: None,
            workflow_name: None,
            trigger_condition: None,
            context: HashMap::new(),
        };
        
        SinkSpec {
            sink_type: SinkType::Stdout,
            config,
            condition: None,
        }
    }
    
    #[tokio::test]
    async fn test_stdout_sink_json() {
        let sink_spec = create_test_sink_spec(Some("json"), Some(true), None);
        let sink = StdoutSink::new("test-sink".to_string(), &sink_spec).unwrap();
        
        let context = json!({
            "workflow": "test-workflow",
            "status": "success"
        });
        
        // This will print to stdout, so we just verify it doesn't error
        assert!(sink.send(context).await.is_ok());
    }
    
    #[tokio::test]
    async fn test_stdout_sink_text_with_template() {
        let template_str = "Workflow: {{ workflow.name }}, Source: {{ source.name }}";
        let sink_spec = create_test_sink_spec(Some("text"), None, Some(template_str));
        let sink = StdoutSink::new("test-sink".to_string(), &sink_spec).unwrap();
        
        let context = json!({
            "workflow": {
                "name": "test-workflow"
            },
            "source": {
                "name": "test-source"
            }
        });
        
        // Test rendering
        assert!(sink.send(context).await.is_ok());
    }
    
    #[tokio::test] 
    async fn test_stdout_sink_text_no_template_prints_json() {
        let sink_spec = create_test_sink_spec(Some("text"), None, None); // No template
        let sink = StdoutSink::new("test-sink".to_string(), &sink_spec).unwrap();
        
        let context = json!({
            "test": "data"
        });
        
        assert!(sink.send(context).await.is_ok());
    }
} 