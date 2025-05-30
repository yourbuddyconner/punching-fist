use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;

use futures::StreamExt;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt},
    runtime::{controller::{Action, Controller}, watcher::Config},
    Client,
};
use serde_json::json;
use tracing::{error, info, warn};

use crate::{
    crd::source::{Source, SourceStatus, Condition},
    sources::WebhookHandler,
    Result, Error,
};

pub struct SourceController {
    client: Client,
    webhook_handler: Arc<WebhookHandler>,
}

impl SourceController {
    pub fn new(client: Client, webhook_handler: Arc<WebhookHandler>) -> Self {
        Self {
            client,
            webhook_handler,
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<()> {
        info!("Starting Source controller");

        let sources: Api<Source> = Api::all(self.client.clone());
        let sources_watcher = Config::default();
        
        Controller::new(sources, sources_watcher)
            .run(Self::reconcile, Self::error_policy, self)
            .for_each(|res| async move {
                match res {
                    Ok((_source, _action)) => {}
                    Err(e) => error!("Reconciliation error: {}", e),
                }
            })
            .await;

        Ok(())
    }

    async fn reconcile(source: Arc<Source>, ctx: Arc<Self>) -> Result<Action> {
        let name = source.name_any();
        let namespace = source.namespace().unwrap_or_default();

        info!("Reconciling Source: {}/{}", namespace, name);

        // Process based on source type
        match &source.spec.source_type {
            crate::crd::source::SourceType::Webhook => {
                if let crate::crd::source::SourceConfig::Webhook(webhook_config) = &source.spec.config {
                    // Register webhook endpoint
                    ctx.webhook_handler.register_webhook(
                        &name,
                        &webhook_config.path,
                        webhook_config.filters.clone(),
                        source.spec.trigger_workflow.clone(),
                        Some(source.spec.trigger_workflow.clone()),
                    ).await?;
                }
            }
            _ => {
                warn!("Source type {:?} not yet implemented", source.spec.source_type);
            }
        }

        // Update status
        let api = Api::<Source>::namespaced(ctx.client.clone(), &namespace);
        let status = SourceStatus {
            ready: true,
            last_event_time: None,
            events_processed: 0,
            conditions: vec![Condition {
                condition_type: "Ready".to_string(),
                status: "True".to_string(),
                reason: "Configured".to_string(),
                message: "Source is configured and ready".to_string(),
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
            Ok(_) => info!("Updated status for Source {}/{}", namespace, name),
            Err(e) => error!("Failed to update status: {}", e),
        }

        Ok(Action::requeue(Duration::from_secs(300))) // Requeue every 5 minutes
    }

    fn error_policy(source: Arc<Source>, err: &Error, _ctx: Arc<Self>) -> Action {
        error!("Error processing Source {}: {}", source.name_any(), err);
        Action::requeue(Duration::from_secs(60))
    }

    pub async fn get_source_by_webhook_path(&self, path: &str) -> Option<Source> {
        None
    }
} 