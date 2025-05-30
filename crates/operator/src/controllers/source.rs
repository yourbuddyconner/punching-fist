use std::sync::Arc;
use kube::{
    Api, Client, 
    api::{Patch, PatchParams},
    runtime::{
        controller::{Action, Controller},
        events::{Recorder, Event, EventType},
        watcher::Config,
    },
    ResourceExt,
};
use futures::StreamExt;
use std::time::Duration;
use tracing::{error, info, warn};
use tokio::sync::RwLock;
use serde_json::json;

use crate::{
    crd::source::{Source, SourceStatus, Condition},
    sources::WebhookHandler,
    Result, OperatorError,
};

pub struct SourceController {
    client: Client,
    webhook_handler: Arc<WebhookHandler>,
    sources: Arc<RwLock<std::collections::HashMap<String, Source>>>,
}

impl SourceController {
    pub fn new(client: Client, webhook_handler: Arc<WebhookHandler>) -> Self {
        Self {
            client,
            webhook_handler,
            sources: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn run(self) -> Result<()> {
        info!("Starting Source controller");

        let api = Api::<Source>::all(self.client.clone());
        let config = Config::default();

        Controller::new(api, config)
            .run(Self::reconcile, Self::error_policy, Arc::new(self))
            .for_each(|res| async move {
                match res {
                    Ok(o) => info!("Reconciled: {:?}", o),
                    Err(e) => error!("Reconcile error: {:?}", e),
                }
            })
            .await;

        Ok(())
    }

    async fn reconcile(source: Arc<Source>, ctx: Arc<Self>) -> Result<Action> {
        let name = source.name_any();
        let namespace = source.namespace().unwrap_or_default();

        info!("Reconciling Source: {}/{}", namespace, name);

        // Update our internal cache
        {
            let mut sources = ctx.sources.write().await;
            sources.insert(source.name_any(), source.as_ref().clone());
        }

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

    fn error_policy(source: Arc<Source>, err: &OperatorError, _ctx: Arc<Self>) -> Action {
        error!("Error processing Source {}: {}", source.name_any(), err);
        Action::requeue(Duration::from_secs(60))
    }

    pub async fn get_source_by_webhook_path(&self, path: &str) -> Option<Source> {
        let sources = self.sources.read().await;
        for source in sources.values() {
            if let crate::crd::source::SourceConfig::Webhook(webhook_config) = &source.spec.config {
                if webhook_config.path == path {
                    return Some(source.clone());
                }
            }
        }
        None
    }
} 