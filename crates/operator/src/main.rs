use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// becomes:
use punching_fist_operator::{
    config::Config,
    kubernetes::KubeClient,
    openhands::OpenHandsClient,
    scheduler::TaskScheduler,
    server::Server,
    store::{self, create_store},
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load configuration
    let config = Config::load()?;
    info!("Loaded configuration: {:?}", config);

    // Initialize store
    let store_boxed = create_store(&config.database).await?;
    store_boxed.init().await?;
    let store: Arc<dyn store::Store> = Arc::from(store_boxed);

    // Initialize KubeClient
    let kube_client = Arc::new(KubeClient::new().await?);

    // Initialize OpenHandsClient
    let openhands_client = Arc::new(OpenHandsClient::new()?);

    // Initialize scheduler
    let scheduler = Arc::new(Mutex::new(TaskScheduler::new(
        kube_client,
        openhands_client,
        config.execution.mode.clone(),
    )));

    // Initialize server
    let server = Server::new(&config, scheduler.clone(), store.clone());

    // Start server
    info!("Starting server on {}", config.server.addr);
    server.start(&config.server.addr).await?;

    Ok(())
} 