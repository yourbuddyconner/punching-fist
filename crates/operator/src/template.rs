//! Template rendering utilities using Tera
//! 
//! This module provides helper functions to convert Go template syntax to Tera syntax
//! and render templates with consistent error handling.

use tera::{Tera, Context};
use serde_json::Value;
use crate::{Result, Error};

/// Convert Go template syntax to Tera syntax
/// 
/// Handles common patterns:
/// - {{ .path.to.value }} -> {{ path.to.value }}
/// - {{ .value | default "default" }} -> {{ value | default(value="default") }}
pub fn convert_go_to_tera(template: &str) -> String {
    let mut converted = template
        .replace("{{ .", "{{ ")
        .replace("{{.", "{{");
    
    // Handle default filter
    let re = regex::Regex::new(r#"\{\{([^}]+)\|\s*default\s+"([^"]+)"\s*\}\}"#).unwrap();
    converted = re.replace_all(&converted, "{{$1| default(value=\"$2\")}}").to_string();
    
    // Handle default filter with single quotes
    let re = regex::Regex::new(r#"\{\{([^}]+)\|\s*default\s+'([^']+)'\s*\}\}"#).unwrap();
    converted = re.replace_all(&converted, "{{$1| default(value=\"$2\")}}").to_string();
    
    // Handle default filter without quotes
    let re = regex::Regex::new(r#"\{\{([^}]+)\|\s*default\s+([^}\s]+)\s*\}\}"#).unwrap();
    converted = re.replace_all(&converted, "{{$1| default(value=\"$2\")}}").to_string();
    
    converted
}

/// Render a template string with the given context
pub fn render_template(template: &str, context: &Value) -> Result<String> {
    // Convert Go template syntax to Tera
    let converted_template = convert_go_to_tera(template);
    
    // Create Tera instance
    let mut tera = Tera::default();
    tera.add_raw_template("template", &converted_template)
        .map_err(|e| Error::Internal(format!("Failed to parse template: {}", e)))?;
    
    // Create Tera context
    let mut tera_context = Context::new();
    
    // Add all fields from the JSON value to the context
    match context {
        Value::Object(map) => {
            for (key, value) in map {
                tera_context.insert(key, &value);
            }
        }
        _ => {
            // If not an object, make it available as "data"
            tera_context.insert("data", &context);
        }
    }
    
    // Render the template
    tera.render("template", &tera_context)
        .map_err(|e| Error::Internal(format!("Failed to render template: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_convert_go_to_tera() {
        let tests = vec![
            ("{{ .name }}", "{{ name }}"),
            ("{{ .path.to.value }}", "{{ path.to.value }}"),
            ("{{.name}}", "{{name}}"),
            ("{{ .name | default \"unknown\" }}", "{{ name | default(value=\"unknown\") }}"),
            ("{{ .count | default 0 }}", "{{ count | default(value=\"0\") }}"),
        ];
        
        for (input, expected) in tests {
            assert_eq!(convert_go_to_tera(input), expected);
        }
    }
    
    #[test]
    fn test_render_template() {
        let context = json!({
            "name": "test-pod",
            "namespace": "default",
            "labels": {
                "app": "test"
            }
        });
        
        let template = "Pod {{ .name }} in namespace {{ .namespace }}";
        let result = render_template(template, &context).unwrap();
        assert_eq!(result, "Pod test-pod in namespace default");
        
        let template_with_default = "Status: {{ .status | default \"unknown\" }}";
        let result = render_template(template_with_default, &context).unwrap();
        assert_eq!(result, "Status: unknown");
    }
} 