use std::sync::Arc;
use tracing::{info, warn};

use punching_fist_operator::{
    config::{Config, TaskExecutionMode},
    controllers::{SourceController, WorkflowController, SinkController},
    server::Server,
    sources::WebhookHandler,
    store::create_store,
    workflow::{WorkflowEngine, StepExecutor},
    Result, Error,
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

    // Initialize Kubernetes client if in Kubernetes mode
    info!("Initializing Kubernetes client...");
    let kube_client = match config.execution.mode {
        TaskExecutionMode::Kubernetes => {
            info!("Initializing Kubernetes client for cluster execution");
            match kube::Client::try_default().await {
                Ok(client) => {
                    info!("Successfully initialized Kubernetes client");
                    client
                }
                Err(e) => {
                    tracing::error!("Failed to initialize Kubernetes client: {}", e);
                    return Err(Error::Kubernetes(e.to_string()));
                }
            }
        }
        TaskExecutionMode::Local => {
            info!("Running in local execution mode, creating in-cluster client anyway for CRD access");
            // Even in local mode, we need a kube client for CRD operations
            match kube::Client::try_default().await {
                Ok(client) => client,
                Err(e) => {
                    warn!("Failed to initialize Kubernetes client in local mode: {}. Some features may not work.", e);
                    // Create a dummy client or handle this case appropriately
                    return Err(Error::Kubernetes(format!("Kubernetes client required even in local mode: {}", e)));
                }
            }
        }
    };

    // Create workflow engine components
    let step_executor = Arc::new(StepExecutor::new(
        kube_client.clone(), 
        config.kube.namespace.clone()
    ));
    let workflow_engine = Arc::new(WorkflowEngine::new(store.clone(), step_executor));
    
    // Create webhook handler with workflow engine
    let webhook_handler = Arc::new(
        WebhookHandler::new(store.clone(), Some(kube_client.clone()))
            .with_workflow_engine(workflow_engine.clone())
    );

    // Start workflow engine
    workflow_engine.clone().start().await;

    // In Kubernetes mode, start controllers
    match config.execution.mode {
        TaskExecutionMode::Kubernetes => {
            info!("Starting in Kubernetes mode");
            
            // Start source controller
            let source_controller = Arc::new(SourceController::new(
                kube_client.clone(),
                webhook_handler.clone(),
            ));
            let controller = source_controller.clone();
            tokio::spawn(async move {
                if let Err(e) = controller.run().await {
                    tracing::error!("Source controller error: {}", e);
                }
            });
            
            // Create sink controller
            let sink_controller = Arc::new(SinkController::new(kube_client.clone()));
            
            // Start sink controller
            let controller = sink_controller.clone();
            tokio::spawn(async move {
                if let Err(e) = controller.run().await {
                    tracing::error!("Sink controller error: {}", e);
                }
            });
            
            // Start workflow controller  
            let workflow_controller = Arc::new(WorkflowController::new(
                kube_client.clone(),
                store.clone(),
                workflow_engine.clone(),
                sink_controller,
            ));
            let controller = workflow_controller.clone();
            tokio::spawn(async move {
                controller.run().await;
            });
        }
        _ => {
            info!("Running in local execution mode, skipping Kubernetes controllers");
        }
    }

    // Initialize server
    info!("Initializing HTTP server...");
    let server = Server::new(&config, store.clone(), webhook_handler.clone());
    let app = server.build_router();

    // Start server
    info!("Starting server on {}", config.server.addr);
    let listener = tokio::net::TcpListener::bind(&config.server.addr)
        .await
        .map_err(|e| {
            tracing::error!("Failed to bind to {}: {}", config.server.addr, e);
            Error::Io(e)
        })?;

    info!("Server listening on {}", config.server.addr);
    
    axum::serve(listener, app)
        .await
        .map_err(|e| {
            tracing::error!("Server error: {}", e);
            Error::Config(format!("Server error: {}", e))
        })?;

    Ok(())
} 