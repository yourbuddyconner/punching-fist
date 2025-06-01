use async_trait::async_trait;
use serde_json::Value;
use crate::crd::sink::{SinkSpec, SinkConfig as CRDSinkConfig}; // Renamed to avoid conflict if SinkConfig struct is also defined here

// Assuming a general Sink trait might look like this.
// Adjust if your Sink trait is defined differently or elsewhere.
// The `new` method signature is updated.
pub trait SinkOutput {
    fn name(&self) -> &str;
    fn new(name_override: Option<String>, spec: &SinkSpec) -> Result<Self, anyhow::Error> where Self: Sized;
    async fn send(&self, context: &Value) -> Result<(), anyhow::Error>;
}

#[derive(Debug)]
pub struct StdoutSink {
    name: String,
    format: String, // "json" or "text"
    pretty: bool,   // For JSON output
    template: Option<String>, // For text output, from SinkConfig.template
}

impl SinkOutput for StdoutSink {
    fn name(&self) -> &str {
        &self.name
    }

    fn new(name_override: Option<String>, spec: &SinkSpec) -> Result<Self, anyhow::Error> {
        // CRD metadata.name is usually used for the resource name.
        // If the sink instance needs an internal name, it should come from spec or an override.
        let name = name_override.unwrap_or_else(|| "stdout_sink_instance".to_string());
        
        let sink_config = &spec.config;

        let format = sink_config.format.as_deref().unwrap_or("json").to_lowercase();
        let pretty = sink_config.pretty.unwrap_or(false);
        // Use the generic template field from SinkConfig
        let template = sink_config.template.clone();

        if format != "json" && format != "text" {
            return Err(anyhow::anyhow!("Invalid format for stdout sink: {}. Must be 'json' or 'text'", format));
        }

        Ok(StdoutSink {
            name,
            format,
            pretty,
            template,
        })
    }

    async fn send(&self, context: &Value) -> Result<(), anyhow::Error> {
        match self.format.as_str() {
            "json" => {
                if self.pretty {
                    match serde_json::to_string_pretty(context) {
                        Ok(json_str) => println!("{}", json_str),
                        Err(e) => {
                            eprintln!("Failed to serialize context to pretty JSON: {}", e);
                            return Err(e.into());
                        }
                    }
                } else {
                    match serde_json::to_string(context) {
                        Ok(json_str) => println!("{}", json_str),
                        Err(e) => {
                            eprintln!("Failed to serialize context to JSON: {}", e);
                            return Err(e.into());
                        }
                    }
                }
            }
            "text" => {
                if let Some(tmpl) = &self.template {
                    // Placeholder for a real template engine (e.g., Tera, Handlebars)
                    // This basic replacement is very limited.
                    // You would pass the `context` (Value) to the template engine.
                    let mut output = tmpl.clone();
                    if let Some(workflow_val) = context.get("workflow") {
                        if let Some(workflow_name) = workflow_val.get("name").and_then(|n| n.as_str()) {
                            output = output.replace("{{ .workflow.name }}", workflow_name);
                        }
                    }
                    if let Some(source_val) = context.get("source") {
                        if let Some(source_name) = source_val.get("name").and_then(|n| n.as_str()) {
                            output = output.replace("{{ .source.name }}", source_name);
                        }
                    }
                    // It's better to use a template engine that can directly take `context` as Value.
                    println!("{}", output);
                } else {
                    // If no template, print the context as a debug string (might be JSON-like)
                    println!("{:?}", context);
                }
            }
            _ => {
                // This case should be caught by the constructor check
                eprintln!("Unsupported format: {}", self.format);
                return Err(anyhow::anyhow!("Unsupported format in send: {}", self.format));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crd::sink::{SinkSpec, SinkType, SinkConfig as CRDSinkConfig}; // Use the renamed import
    use std::collections::HashMap;
    use serde_json::json;

    fn create_test_sink_spec(format: Option<&str>, pretty: Option<bool>, template: Option<&str>) -> SinkSpec {
        let mut sink_config = CRDSinkConfig { // Now refers to the struct from crd::sink
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
            template: template.map(String::from),
            format: format.map(String::from),
            pretty,
        };

        SinkSpec {
            sink_type: SinkType::Stdout, // Assuming SinkType::Stdout is defined
            config: sink_config,
            condition: None,
        }
    }

    #[tokio::test]
    async fn test_stdout_sink_json_not_pretty() {
        let sink_spec = create_test_sink_spec(Some("json"), Some(false), None);
        let sink = StdoutSink::new(Some("test_sink".to_string()), &sink_spec).unwrap();
        let context = json!({ "workflow": { "name": "test_workflow" }, "status": "completed" });
        assert!(sink.send(&context).await.is_ok());
    }

    #[tokio::test]
    async fn test_stdout_sink_json_pretty() {
        let sink_spec = create_test_sink_spec(Some("json"), Some(true), None);
        let sink = StdoutSink::new(Some("test_sink".to_string()), &sink_spec).unwrap();
        let context = json!({ "workflow": { "name": "test_workflow" }, "status": "completed" });
        assert!(sink.send(&context).await.is_ok());
    }

    #[tokio::test]
    async fn test_stdout_sink_text_with_template() {
        let template_str = "Workflow: {{ .workflow.name }}, Source: {{ .source.name }}";
        let sink_spec = create_test_sink_spec(Some("text"), None, Some(template_str));
        let sink = StdoutSink::new(Some("test_sink".to_string()), &sink_spec).unwrap();
        let context = json!({ "workflow": { "name": "pipeline_alpha" }, "source": { "name": "alertmanager" } });
        assert!(sink.send(&context).await.is_ok());
    }

    #[tokio::test]
    async fn test_stdout_sink_text_no_template_prints_debug() {
        let sink_spec = create_test_sink_spec(Some("text"), None, None); // No template
        let sink = StdoutSink::new(Some("test_sink".to_string()), &sink_spec).unwrap();
        let context = json!({ "workflow": { "name": "default_text_test" }});
        assert!(sink.send(&context).await.is_ok()); // Should print debug format of context
    }

    #[test]
    fn test_stdout_sink_invalid_format_in_spec() {
        let sink_spec = create_test_sink_spec(Some("xml"), None, None);
        assert!(StdoutSink::new(Some("test_sink".to_string()), &sink_spec).is_err());
    }

    #[test]
    fn test_stdout_sink_default_format_is_json() {
        let sink_spec = create_test_sink_spec(None, None, None); // format is None
        let sink = StdoutSink::new(Some("test_sink".to_string()), &sink_spec).unwrap();
        assert_eq!(sink.format, "json");
        assert_eq!(sink.pretty, false); // Default pretty is false
    }
} 