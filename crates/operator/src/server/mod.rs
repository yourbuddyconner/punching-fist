mod routes;

use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower_http::{
    trace::TraceLayer,
    services::fs::ServeDir,
};
use tracing::info;

use crate::{
    config::Config,
    sources::WebhookHandler,
    store::Store,
    // Removed old imports: AlertRecord, TaskRecord, TaskStatus
};

pub struct Server {
    store: Arc<dyn Store>,
    pub webhook_handler: Arc<WebhookHandler>,
}

impl Server {
    pub fn new(
        _config: &Config, 
        store: Arc<dyn Store>,
        webhook_handler: Arc<WebhookHandler>,
    ) -> Self {
        Self { store, webhook_handler }
    }

    pub fn build_router(self) -> Router {
        let state = Arc::new(self);

        // Get static file path from environment variable or use defaults
        let static_path = std::env::var("STATIC_FILE_PATH")
            .unwrap_or_else(|_| {
                // If env var not set, check for common paths
                if std::path::Path::new("crates/operator/static").exists() {
                    "crates/operator/static".to_string()
                } else if std::path::Path::new("/usr/local/share/punching-fist/static").exists() {
                    "/usr/local/share/punching-fist/static".to_string()
                } else {
                    // Fallback to development path
                    "crates/operator/static".to_string()
                }
            });

        info!("Serving static files from: {}", static_path);

        Router::new()
            .route("/", get(routes::root))
            .route("/health", get(routes::health))
            // Alert endpoints
            .route("/alerts", post(routes::create_alert))
            .route("/alerts", get(routes::list_alerts))
            .route("/alerts/{id}", get(routes::get_alert))
            // Workflow endpoints
            .route("/workflows", get(routes::list_workflows))
            .route("/workflows/{id}", get(routes::get_workflow))
            .route("/workflows/{id}/steps", get(routes::list_workflow_steps))
            .route("/workflows/{id}/outputs", get(routes::list_workflow_outputs))
            // Source event endpoints
            .route("/source-events", get(routes::list_source_events))
            // Webhook and metrics
            .route("/webhook/{*path}", post(routes::webhook_alerts))
            .route("/metrics", get(routes::metrics))
            // Serve UI at /ui and /ui/* 
            .nest_service("/ui", ServeDir::new(static_path))
            .layer(TraceLayer::new_for_http())
            .with_state(state)
    }
} 