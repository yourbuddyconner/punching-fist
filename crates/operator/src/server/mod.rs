mod routes;
mod receivers;

use axum::{
    extract::State,
    routing::{get, post},
    Router,
};
use std::sync::{Arc, Mutex};
use tower_http::trace::TraceLayer;

use crate::{
    config::Config,
    scheduler::TaskScheduler,
    sources::WebhookHandler,
    store::Store,
    // Removed old imports: AlertRecord, TaskRecord, TaskStatus
};

pub use receivers::{Alert, PrometheusReceiver};

pub struct Server {
    scheduler: Arc<Mutex<TaskScheduler>>,
    store: Arc<dyn Store>,
    pub webhook_handler: Arc<WebhookHandler>,
}

impl Server {
    pub fn new(
        _config: &Config, 
        scheduler: Arc<Mutex<TaskScheduler>>, 
        store: Arc<dyn Store>,
        webhook_handler: Arc<WebhookHandler>,
    ) -> Self {
        Self { scheduler, store, webhook_handler }
    }

    pub fn build_router(self) -> Router {
        let state = Arc::new(self);

        Router::new()
            .route("/health", get(routes::health))
            .route("/alerts", post(routes::create_alert))
            .route("/alerts/:id", get(routes::get_alert))
            .route("/alerts", get(routes::list_alerts))
            // TODO: Phase 1 - update task routes to workflow routes
            //.route("/tasks", post(routes::create_task))
            //.route("/tasks/:id", get(routes::get_task))
            //.route("/tasks", get(routes::list_tasks))
            .route("/webhook/*path", post(routes::webhook_alerts))
            .route("/metrics", get(routes::metrics))
            .layer(TraceLayer::new_for_http())
            .with_state(state)
    }
} 