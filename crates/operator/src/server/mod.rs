mod receivers;
mod routes;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use crate::{
    scheduler::TaskScheduler,
    Result,
};
use receivers::{AlertReceiver, PrometheusReceiver, PrometheusConfig};

pub struct Server {
    scheduler: Arc<Mutex<TaskScheduler>>,
    receiver: Arc<dyn AlertReceiver>,
}

impl Server {
    pub fn new(scheduler: Arc<Mutex<TaskScheduler>>) -> Self {
        let config = PrometheusConfig::default();
        let receiver = Arc::new(PrometheusReceiver::new(config));
        
        Self { 
            scheduler,
            receiver,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let app = Router::new()
            .route("/webhook/alerts", post(routes::alert_handler))
            .route("/health", get(routes::health_check))
            .route("/metrics", get(routes::metrics))
            .with_state(self.scheduler.clone())
            .with_state(self.receiver.clone());

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
        info!("Starting server on {}", addr);

        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .map_err(|e| crate::OperatorError::Config(e.to_string()))?;

        Ok(())
    }
} 