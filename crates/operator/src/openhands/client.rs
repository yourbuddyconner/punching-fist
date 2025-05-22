use reqwest::Client;
use crate::{
    Result,
    OperatorError,
};

pub struct OpenHandsClient {
    api_key: String,
    client: Client,
}

impl OpenHandsClient {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("OPENHANDS_API_KEY")
            .map_err(|_| OperatorError::Config("OPENHANDS_API_KEY not set".to_string()))?;

        let client = Client::new();

        Ok(Self {
            api_key,
            client,
        })
    }

    pub async fn process_task(&self, task: &crate::Task) -> Result<()> {
        use tokio::process::Command;
        use std::process::Stdio;

        // Build the headless OpenHands invocation as documented at
        // https://docs.all-hands.dev/modules/usage/how-to/headless-mode
        // We execute the equivalent of:
        //   poetry run python -m openhands.core.main -t "<prompt>"
        // but without relying on Poetry. Instead, we assume that the
        // `python` executable available in PATH has the `openhands` package
        // installed (either system-wide or inside a virtualenv/container).

        // NOTE: If a specific model was requested in the task we expose it via
        // `LLM_MODEL`. The API key is always provided via `LLM_API_KEY` and we
        // default to logging all events to aid debugging.

        let mut cmd = Command::new("python");
        cmd.arg("-m")
            .arg("openhands.core.main")
            .arg("-t")
            .arg(&task.prompt)
            // Pass the API key we captured at client construction time.
            .env("LLM_API_KEY", &self.api_key)
            // Enable detailed logs so operators can inspect failures.
            .env("LOG_ALL_EVENTS", "true")
            // Forward the chosen model when provided, otherwise the OpenHands
            // default (configured via env or config.toml) will be used.
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(model) = &task.model {
            cmd.env("LLM_MODEL", model);
        }

        // Spawn the process and capture the output so we can bubble up rich
        // error information if the run fails.
        let mut child = cmd
            .spawn()
            .map_err(|e| OperatorError::OpenHands(format!(
                "failed to spawn OpenHands process: {e}"
            )))?;

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| OperatorError::OpenHands(format!(
                "failed to wait for OpenHands process: {e}"
            )))?;

        if !output.status.success() {
            return Err(OperatorError::OpenHands(format!(
                "OpenHands exited with status {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        Ok(())
    }
} 