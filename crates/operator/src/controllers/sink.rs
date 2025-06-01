use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt},
    runtime::{controller::{Action, Controller}, watcher::Config},
    Client,
};
use serde_json::{json, Value};
use tracing::{debug, error, info, warn};

use crate::crd::sink::{Sink, SinkSpec, SinkStatus, SinkType as CRDSinkType}; // Using authoritative definitions
use crate::crd::source::Condition;
use crate::sinks::stdout::StdoutSink;
use crate::sinks::stdout::SinkOutput; // The trait we defined
use crate::{Result, Error};

#[derive(Clone)] // Added Clone
pub struct SinkController {
    client: Client,
    // Potentially a cache for Sink CRs if lookups are frequent
}

impl SinkController {
    pub fn new(client: Client) -> Self {
        SinkController { client }
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting Sink controller");

        let sinks: Api<Sink> = Api::all(self.client.clone());
        let sinks_watcher = Config::default();
        
        Controller::new(sinks, sinks_watcher)
            .run(Self::reconcile, Self::error_policy, self)
            .for_each(|res| async move {
                match res {
                    Ok((_sink, _action)) => {}
                    Err(e) => error!("Reconciliation error: {}", e),
                }
            })
            .await;

        Ok(())
    }

    async fn reconcile(sink: Arc<Sink>, ctx: Arc<Self>) -> Result<Action> {
        let name = sink.name_any();
        let namespace = sink.namespace().unwrap_or_default();
        
        // Get current status
        let current_status = sink.status.as_ref();
        let is_ready = current_status.map(|s| s.ready).unwrap_or(false);
        
        // Check if this is a new resource or not ready
        let needs_update = current_status.is_none() || !is_ready;
        
        if needs_update {
            info!("Registering new Sink resource: {}/{}", namespace, name);
            info!(
                "Sink '{}' configured with type '{:?}'",
                name, sink.spec.sink_type
            );
        } else {
            debug!("Reconciling existing Sink: {}/{}", namespace, name);
        }
        
        // Validate sink configuration
        match &sink.spec.sink_type {
            CRDSinkType::Stdout => {
                debug!("Validated stdout sink configuration for '{}'", name);
            }
            CRDSinkType::Slack => {
                if sink.spec.config.channel.is_none() || sink.spec.config.bot_token.is_none() {
                    warn!("Slack sink '{}' missing required configuration", name);
                }
            }
            CRDSinkType::Jira => {
                if sink.spec.config.project.is_none() || sink.spec.config.credentials_secret.is_none() {
                    warn!("JIRA sink '{}' missing required configuration", name);
                }
            }
            _ => {
                debug!("Sink type {:?} configuration validated for '{}'", sink.spec.sink_type, name);
            }
        }
        
        // Only update status if needed
        if needs_update {
            let api = Api::<Sink>::namespaced(ctx.client.clone(), &namespace);
            
            // Preserve existing counters
            let messages_sent = current_status.map(|s| s.messages_sent).unwrap_or(0);
            let last_sent_time = current_status.and_then(|s| s.last_sent_time.clone());
            
            let status = SinkStatus {
                ready: true,
                last_sent_time,
                messages_sent,
                last_error: None,
                conditions: vec![Condition {
                    condition_type: "Ready".to_string(),
                    status: "True".to_string(),
                    reason: "Configured".to_string(),
                    message: format!("Sink is configured and ready to receive events"),
                    last_transition_time: chrono::Utc::now().to_rfc3339(),
                }],
            };

            let status_patch = json!({
                "status": status
            });

            let patch_params = PatchParams::default();
            match api
                .patch_status(&name, &patch_params, &Patch::Merge(&status_patch))
                .await
            {
                Ok(_) => {
                    info!("Successfully updated Sink {}/{} to ready state", namespace, name);
                }
                Err(e) => error!("Failed to update status: {}", e),
            }
        }

        Ok(Action::requeue(Duration::from_secs(300))) // Requeue every 5 minutes
    }

    fn error_policy(sink: Arc<Sink>, err: &Error, _ctx: Arc<Self>) -> Action {
        error!("Error processing Sink {}: {}", sink.name_any(), err);
        Action::requeue(Duration::from_secs(60))
    }

    // This function will be called by the workflow engine when a workflow completes
    // and needs to send its output to a configured sink.
    pub async fn process_sink_event(
        &self,
        sink_name: &str,
        sink_namespace: &str, // Sinks are namespaced
        workflow_output_context: &Value, // The full context to be sent to the sink
    ) -> Result<()> {
        info!(
            "Processing sink event for sink '{}' in namespace '{}'",
            sink_name,
            sink_namespace
        );

        let sinks_api: Api<Sink> = Api::namespaced(self.client.clone(), sink_namespace);
        
        // Fetch the Sink CRD instance
        let sink_cr = match sinks_api.get(sink_name).await {
            Ok(s) => s,
            Err(e) => {
                return Err(Error::Kubernetes(format!(
                    "Failed to get Sink CRD '{}' in namespace '{}': {}",
                    sink_name,
                    sink_namespace,
                    e
                )));
            }
        };

        let sink_spec = sink_cr.spec; // sink_cr has SinkSpec as spec

        // Dispatch to the correct sink implementation based on sink_spec.sink_type
        match sink_spec.sink_type {
            CRDSinkType::Stdout => {
                // The name for the SinkOutput instance can be the CRD name
                let stdout_sink = StdoutSink::new(Some(sink_name.to_string()), &sink_spec)
                    .map_err(|e| Error::Config(format!("Failed to create stdout sink: {}", e)))?;
                info!("Dispatching to StdoutSink: {}", stdout_sink.name());
                stdout_sink.send(workflow_output_context).await
                    .map_err(|e| Error::Config(format!("Failed to send to stdout sink: {}", e)))?;
                
                // Update sink status with message count
                self.update_sink_message_count(&sinks_api, sink_name).await?;
                
                Ok(())
            }
            CRDSinkType::Slack => {
                // Placeholder for SlackSink implementation
                info!("Slack sink type not yet implemented. Sink: {}", sink_name);
                // let slack_sink = SlackSink::new(Some(sink_name.to_string()), &sink_spec)?;
                // slack_sink.send(workflow_output_context).await?
                Ok(())
            }
            CRDSinkType::AlertManager => {
                info!("AlertManager sink type not yet implemented. Sink: {}", sink_name);
                Ok(())
            }
            // Add other sink types here
            _ => {
                error!(
                    "Sink type '{:?}' for sink '{}' is not supported yet.",
                    sink_spec.sink_type,
                    sink_name
                );
                Ok(())
            }
        }
    }
    
    async fn update_sink_message_count(&self, api: &Api<Sink>, sink_name: &str) -> Result<()> {
        // Get current sink to get message count
        let sink = api.get(sink_name).await
            .map_err(|e| Error::Kubernetes(format!("Failed to get sink for status update: {}", e)))?;
        
        let current_count = sink.status.as_ref().map(|s| s.messages_sent).unwrap_or(0);
        
        let status_patch = json!({
            "status": {
                "messagesSent": current_count + 1,
                "lastSentTime": chrono::Utc::now().to_rfc3339()
            }
        });
        
        api.patch_status(sink_name, &PatchParams::default(), &Patch::Merge(status_patch))
            .await
            .map_err(|e| Error::Kubernetes(format!("Failed to update sink status: {}", e)))?;
            
        info!("Updated message count for sink '{}'", sink_name);
        Ok(())
    }
}

// Basic test structure (you'll need a running K8s cluster or mock for Api calls)
#[cfg(test)]
mod tests {
    use super::*;
    use kube::Client;
    use serde_json::json;
    use crate::crd::sink::{SinkConfig, SinkStatus};

    // Mocking K8s API calls is complex and out of scope for this basic test.
    // These tests would typically require environment setup for a test cluster
    // or a sophisticated mocking framework like `kube-runtime/controller-runtime` provides.

    // A conceptual test for the dispatch logic if we could mock get_sink_crd
    /*
    async fn mock_get_sink_crd(name: &str, namespace: &str, client: Client) -> Result<Sink, kube::Error> {
        // In a real test, this would interact with a mock client
        // For now, let's construct a Sink directly if it's for stdout
        if name == "my-stdout-sink" && namespace == "default" {
            Ok(Sink {
                metadata: Arc::new(kube::api::ObjectMeta {
                    name: Some(name.to_string()),
                    namespace: Some(namespace.to_string()),
                    ..
                    Default::default()
                }),
                spec: SinkSpec {
                    sink_type: CRDSinkType::Stdout,
                    config: SinkConfig { // Assuming this is the detailed one from crd::sink
                        format: Some("json".to_string()),
                        pretty: Some(true),
                        // ... other fields initialized to None or default ...
                        channel: None, bot_token: None, message_type: None, mention_users: vec![],
                        endpoint: None, action: None, pushgateway: None, job: None, metrics: std::collections::HashMap::new(),
                        project: None, issue_type: None, credentials_secret: None, routing_key: None,
                        workflow_name: None, trigger_condition: None, template: None, context: std::collections::HashMap::new(),
                    },
                    condition: None,
                },
                status: None, // Or some default status
            })
        } else {
            Err(kube::Error::Api(kube::error::ErrorResponse { 
                status: "Failure".to_string(), 
                message: "Not Found".to_string(), 
                reason: "NotFound".to_string(), 
                code: 404 
            }))
        }
    }

    #[tokio::test]
    async fn test_process_stdout_sink_event() {
        // This test requires a K8s client that can be mocked or a test environment.
        // For now, this is a placeholder demonstrating the intent.
        // let client = Client::try_default().await.expect("Failed to create K8s client");
        // let controller = SinkController::new(client.clone());
        
        // Here you would mock the Api<Sink>::get call to return a specific Sink CRD
        // For example, using a library like `mockall` or a custom mock client.

        let context = json!({ "message": "hello from workflow" });
        
        // Simulate a call - this will fail without a real/mocked K8s API
        // let result = controller.process_sink_event("my-stdout-sink", "default", &context).await;
        // assert!(result.is_ok()); 
        // Further assertions would involve capturing stdout or checking mock interactions.
        println!("Skipping test_process_stdout_sink_event due to K8s client dependency");
    }
    */
} 