use tokio::process::Command;
use std::process::Stdio;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use crate::{
    Result,
    OperatorError,
    config::{OpenHandsConfig, TaskExecutionMode},
    store::{Store, TaskStatus},
};

pub struct OpenHandsClient {
    config: OpenHandsConfig,
    execution_mode: TaskExecutionMode,
    store: Arc<dyn Store>,
}

impl OpenHandsClient {
    pub fn new(config: OpenHandsConfig, execution_mode: TaskExecutionMode, store: Arc<dyn Store>) -> Result<Self> {
        // OpenHands requires an API key
        if config.api_key.is_empty() {
            return Err(OperatorError::Config(
                "LLM_API_KEY environment variable must be set".to_string(),
            ));
        }

        // Only check Docker availability in Local execution mode
        // In Kubernetes mode, Docker is only used as a fallback
        if matches!(execution_mode, TaskExecutionMode::Local) {
            // Check if Docker is available and working by running a simple command
            let docker_check = std::process::Command::new("docker")
                .arg("version")
                .arg("--format")
                .arg("{{.Server.Version}}")
                .output();

            match docker_check {
                Ok(output) if output.status.success() => {
                    let docker_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    tracing::info!("Docker is available, server version: {}", docker_version);
                }
                Ok(output) => {
                    let error = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(OperatorError::Config(
                        format!("Docker command failed: {}", error)
                    ));
                }
                Err(e) => {
                    return Err(OperatorError::Config(
                        format!("Docker not found or not accessible: {}. Ensure Docker is installed and running.", e)
                    ));
                }
            }
        } else {
            tracing::info!("Running in Kubernetes execution mode - Docker check skipped (will be used as fallback only)");
        }

        Ok(Self {
            config,
            execution_mode,
            store,
        })
    }

    pub async fn process_task(&self, task: &crate::Task, task_id: Uuid) -> Result<()> {
        // Update task status to Running and set started_at timestamp
        let started_at = Utc::now();
        if let Err(e) = self.store.update_task_completion(
            task_id,
            TaskStatus::Running,
            Some(started_at),
            None,
            None,
        ).await {
            tracing::warn!("Failed to update task status to Running: {}", e);
        }

        // Build the Docker command for OpenHands headless execution
        // Following the documentation at https://docs.all-hands.dev/modules/usage/how-to/headless-mode
        
        // Generate a unique container name with timestamp
        let timestamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
        let container_name = format!("openhands-task-{}", timestamp);

        // Get current user ID for proper permissions
        let user_id_output = std::process::Command::new("id")
            .arg("-u")
            .output()
            .map_err(|e| OperatorError::OpenHands(format!("Failed to get user ID: {}", e)))?
            .stdout;
        let user_id = String::from_utf8_lossy(&user_id_output).trim().to_string();

        // Create the OpenHands state directory if it doesn't exist
        let state_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()) + "/.openhands-state";
        if let Err(e) = std::fs::create_dir_all(&state_dir) {
            tracing::warn!("Failed to create OpenHands state directory {}: {}", state_dir, e);
        }

        let mut cmd = Command::new("docker");
        cmd.arg("run")
            .arg("-i")  // Interactive mode (without -t for non-TTY environment)
            .arg("--pull=always")
            .arg("--rm");  // Remove container after execution

        // Use host networking in local mode for easier access to local services
        match self.execution_mode {
            TaskExecutionMode::Local => {
                // cmd.arg("--network=host");
                tracing::debug!("Using host networking for local execution mode");
            }
            TaskExecutionMode::Kubernetes => {
                // In Kubernetes mode, use default networking with host gateway
                cmd.arg("--add-host").arg("host.docker.internal:host-gateway");
                tracing::debug!("Using bridge networking with host gateway for Kubernetes execution mode");
            }
        }

        cmd
            // Environment variables
            .arg("-e").arg("SANDBOX_RUNTIME_CONTAINER_IMAGE=docker.all-hands.dev/all-hands-ai/runtime:0.39-nikolaik")
            .arg("-e").arg(format!("SANDBOX_USER_ID={}", user_id))
            .arg("-e").arg(format!("LLM_API_KEY={}", self.config.api_key))
            .arg("-e").arg("LOG_ALL_EVENTS=true");

        // Set the model if specified in the task, otherwise use config default
        let model = task.model.as_ref().unwrap_or(&self.config.default_model);
        cmd.arg("-e").arg(format!("LLM_MODEL={}", model));

        cmd
            // Volume mounts
            .arg("-v").arg("/var/run/docker.sock:/var/run/docker.sock")
            .arg("-v").arg(format!("{}:/.openhands-state", state_dir))
            // Container name
            .arg("--name").arg(&container_name)
            // Docker image
            .arg("docker.all-hands.dev/all-hands-ai/openhands:0.39")
            // Command to run
            .arg("python")
            .arg("-m")
            .arg("openhands.core.main")
            .arg("-t")
            .arg(&task.prompt)
            // Capture output
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Log the full Docker command in local mode for debugging
        if matches!(self.execution_mode, TaskExecutionMode::Local) {
            tracing::debug!("Docker command args in local mode: {:?}", cmd.as_std().get_args().collect::<Vec<_>>());
        }

        tracing::info!("Starting OpenHands Docker container: {} (Task ID: {})", container_name, task_id);
        tracing::debug!("OpenHands task prompt: {}", task.prompt);

        // Spawn the Docker process
        let child = cmd
            .spawn()
            .map_err(|e| OperatorError::OpenHands(format!(
                "Failed to spawn OpenHands Docker container: {}. Ensure Docker is running and accessible.", e
            )))?;

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| OperatorError::OpenHands(format!(
                "Failed to wait for OpenHands Docker container: {}", e
            )))?;

        // Log the output for debugging
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // In local mode, also write all output to a log file
        if matches!(self.execution_mode, TaskExecutionMode::Local) {
            let log_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()) + "/.openhands-logs";
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                tracing::warn!("Failed to create OpenHands log directory {}: {}", log_dir, e);
            } else {
                let log_file_path = format!("{}/openhands-{}.log", log_dir, container_name);
                let log_content = format!(
                    "=== OpenHands Container Output ===\n\
                     Container: {}\n\
                     Task ID: {}\n\
                     Task: {}\n\
                     Model: {}\n\
                     Timestamp: {}\n\
                     Exit Status: {}\n\n\
                     === STDOUT ===\n{}\n\n\
                     === STDERR ===\n{}\n",
                    container_name,
                    task_id,
                    task.prompt,
                    task.model.as_ref().unwrap_or(&self.config.default_model),
                    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
                    output.status,
                    stdout.trim(),
                    stderr.trim()
                );
                
                if let Err(e) = std::fs::write(&log_file_path, log_content) {
                    tracing::warn!("Failed to write OpenHands log file {}: {}", log_file_path, e);
                } else {
                    tracing::info!("OpenHands container output saved to: {}", log_file_path);
                }
            }
        }
        
        if !stdout.trim().is_empty() {
            tracing::info!("OpenHands stdout: {}", stdout.trim());
        }
        if !stderr.trim().is_empty() {
            tracing::debug!("OpenHands stderr: {}", stderr.trim());
        }

        // Update task completion status in database
        let completed_at = Utc::now();
        let (final_status, error_message) = if output.status.success() {
            (TaskStatus::Succeeded, None)
        } else {
            (TaskStatus::Failed, Some(format!(
                "OpenHands Docker container exited with status {}: {}",
                output.status,
                stderr.trim()
            )))
        };

        if let Err(e) = self.store.update_task_completion(
            task_id,
            final_status,
            Some(started_at),
            Some(completed_at),
            error_message.clone(),
        ).await {
            tracing::warn!("Failed to update task completion status: {}", e);
        }

        if !output.status.success() {
            return Err(OperatorError::OpenHands(format!(
                "OpenHands Docker container exited with status {}: {}",
                output.status,
                stderr.trim()
            )));
        }

        tracing::info!("OpenHands task completed successfully in container: {} (Task ID: {})", container_name, task_id);
        Ok(())
    }
} 