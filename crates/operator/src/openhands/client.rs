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
        // The actual task processing is done by the OpenHands container
        // This client is mainly for configuration and monitoring
        Ok(())
    }
} 