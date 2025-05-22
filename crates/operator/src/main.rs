use punching_fist_operator::{
    kubernetes::KubeClient,
    openhands::OpenHandsClient,
    server::Server,
    scheduler::TaskScheduler,
    Result,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_names(true)
        .with_ansi(true)
        .pretty()
        .init();

    info!("Starting Punching Fist Operator");

    // Initialize Kubernetes client
    let kube_client = KubeClient::new().await?;
    let kube_client = Arc::new(kube_client);

    // Initialize OpenHands client
    let openhands_client = OpenHandsClient::new()?;
    let openhands_client = Arc::new(openhands_client);

    // Initialize task scheduler
    let scheduler = TaskScheduler::new(kube_client.clone(), openhands_client.clone());
    let scheduler = Arc::new(Mutex::new(scheduler));

    // Initialize and start the server
    let server = Server::new(scheduler.clone());
    server.start().await?;

    Ok(())
} 