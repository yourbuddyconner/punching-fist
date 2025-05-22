mod routes;
mod receivers;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::net::SocketAddr;
use tracing;

use crate::{
    config::Config,
    scheduler::TaskScheduler,
    store::{AlertRecord, Store, TaskRecord, TaskStatus},
    OperatorError,
};
use receivers::{AlertReceiver, PrometheusReceiver, PrometheusConfig};

pub use receivers::Alert;

pub struct Server {
    scheduler: Arc<Mutex<TaskScheduler>>,
    receiver: Arc<dyn AlertReceiver>,
    store: Arc<dyn Store>,
}

impl Server {
    pub fn new(config: &Config, scheduler: Arc<Mutex<TaskScheduler>>, store: Arc<dyn Store>) -> Self {
        let receiver = Arc::new(PrometheusReceiver::new(PrometheusConfig::default()));
        Self {
            scheduler,
            receiver,
            store,
        }
    }

    pub async fn start(&self, addr: &str) -> crate::Result<()> {
        let app = Router::new()
            .route("/health", get(routes::health_check))
            .route("/metrics", get(routes::metrics))
            .route("/alerts", post(routes::alert_handler))
            .with_state(Arc::new(ServerState {
                scheduler: self.scheduler.clone(),
                receiver: self.receiver.clone(),
                store: self.store.clone(),
            }));

        let addr = match addr.parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                tracing::error!("Failed to parse address {}: {}", addr, e);
                return Err(OperatorError::Config(format!("Invalid address: {}", e)));
            }
        };

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(e) => {
                tracing::error!("Failed to bind to {}: {}", addr, e);
                return Err(OperatorError::Config(format!("Failed to bind to address: {}", e)));
            }
        };

        tracing::info!("Server listening on {}", addr);
        
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("Server error: {}", e);
            return Err(OperatorError::Config(format!("Server error: {}", e)));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct ServerState {
    scheduler: Arc<Mutex<TaskScheduler>>,
    receiver: Arc<dyn AlertReceiver>,
    store: Arc<dyn Store>,
} 