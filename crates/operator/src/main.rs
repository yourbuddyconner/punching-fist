use std::sync::Arc;
use std::sync::Mutex;
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

    info!("Starting punching-fist-operator Phase 1...");

    // Load configuration
    info!("Loading configuration...");
    let config = match Config::load() {
        Ok(config) => {
            info!("Successfully loaded configuration");
            config
        }
        Err(e) => {
            tracing::error!("Failed to load configuration: {}", e);
            return Err(e);
        }
    };

    // Initialize store
    info!("Initializing database store...");
    let store = match create_store(&config.database).await {
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
    if let Err(e) = store.init().await {
        tracing::error!("Failed to initialize database: {}", e);
        return Err(e);
    }
    info!("Database initialized successfully");

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

    // Initialize OpenHandsClient (will be replaced in Phase 1 with LLM agent runtime)
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

    // Initialize scheduler (will be replaced with workflow engine in Phase 1)
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
    let app = server.build_router();

    // Start server
    info!("Starting server on {}", config.server.addr);
    let listener = tokio::net::TcpListener::bind(&config.server.addr)
        .await
        .map_err(|e| {
            tracing::error!("Failed to bind to {}: {}", config.server.addr, e);
            punching_fist_operator::OperatorError::Io(e)
        })?;

    info!("Server listening on {}", config.server.addr);
    
    axum::serve(listener, app)
        .await
        .map_err(|e| {
            tracing::error!("Server error: {}", e);
            punching_fist_operator::OperatorError::Config(format!("Server error: {}", e))
        })?;

    Ok(())
} 