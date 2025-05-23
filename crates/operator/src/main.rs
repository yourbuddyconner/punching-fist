use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

// becomes:
use punching_fist_operator::{
    config::{Config, TaskExecutionMode},
    kubernetes::KubeClient,
    openhands::OpenHandsClient,
    scheduler::TaskScheduler,
    server::Server,
    store::{self, create_store},
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with more verbose configuration
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    info!("Starting punching-fist-operator...");

    // Load configuration
    info!("Loading configuration...");
    let config = match Config::load() {
        Ok(config) => {
            info!("Successfully loaded configuration: {:?}", config);
            config
        }
        Err(e) => {
            tracing::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    // Initialize store
    info!("Initializing database store...");
    let store_boxed = match create_store(&config.database).await {
        Ok(store) => {
            info!("Successfully created database store");
            store
        }
        Err(e) => {
            tracing::error!("Failed to create database store: {}", e);
            return Err(e);
        }
    };

    info!("Initializing database...");
    if let Err(e) = store_boxed.init().await {
        tracing::error!("Failed to initialize database: {}", e);
        return Err(e);
    }
    info!("Database initialized successfully");

    let store: Arc<dyn store::Store> = Arc::from(store_boxed);

    // Initialize KubeClient only if we're in Kubernetes execution mode
    let kube_client = match config.execution.mode {
        TaskExecutionMode::Kubernetes => {
            info!("Initializing Kubernetes client for cluster execution");
            match KubeClient::new().await {
                Ok(client) => {
                    info!("Successfully initialized Kubernetes client");
                    Some(Arc::new(client))
                }
                Err(e) => {
                    tracing::error!("Failed to initialize Kubernetes client: {}", e);
                    return Err(e);
                }
            }
        }
        TaskExecutionMode::Local => {
            info!("Running in local execution mode, skipping Kubernetes client initialization");
            None
        }
    };

    // Initialize OpenHandsClient
    info!("Initializing OpenHands client...");
    let openhands_client = match OpenHandsClient::new(config.openhands.clone(), config.execution.mode.clone(), store.clone()) {
        Ok(client) => {
            info!("Successfully initialized OpenHands client");
            Arc::new(client)
        }
        Err(e) => {
            tracing::error!("Failed to initialize OpenHands client: {}", e);
            return Err(e);
        }
    };

    // Initialize scheduler
    info!("Initializing task scheduler...");
    let scheduler = Arc::new(Mutex::new(TaskScheduler::new(
        kube_client,
        openhands_client,
        store.clone(),
        config.execution.mode.clone(),
    )));
    info!("Task scheduler initialized successfully");

    // Initialize server
    info!("Initializing HTTP server...");
    let server = Server::new(&config, scheduler.clone(), store.clone());

    // Start server
    info!("Starting server on {}", config.server.addr);
    if let Err(e) = server.start(&config.server.addr).await {
        tracing::error!("Server failed to start: {}", e);
        return Err(e);
    }

    Ok(())
} 