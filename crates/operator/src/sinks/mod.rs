pub mod stdout;
// pub mod slack; // Keep slack for future, but stdout is the focus
// pub mod alertmanager;
// pub mod templates;

// Potentially a trait or enum that all sinks implement/are part of
// For example:
// use self::stdout::StdoutSink;
// use self::slack::SlackSink;
// use self::alertmanager::AlertManagerSink;
// use crate::crd::sink::SinkConfig;
// use serde_json::Value;
// use async_trait::async_trait;

/*
#[async_trait]
pub trait Sink {
    fn name(&self) -> &str;
    async fn send(&self, context: &Value) -> Result<(), anyhow::Error>;
}

pub enum AllSinks {
    Stdout(StdoutSink),
    Slack(SlackSink),
    AlertManager(AlertManagerSink),
}

impl AllSinks {
    pub fn new(config: &SinkConfig) -> Result<Self, anyhow::Error> {
        match config.sink_type.to_lowercase().as_str() {
            "stdout" => Ok(AllSinks::Stdout(StdoutSink::new(config)?)),
            "slack" => Ok(AllSinks::Slack(SlackSink::new(config)?)),
            "alertmanager" => Ok(AllSinks::AlertManager(AlertManagerSink::new(config)?)),
            _ => Err(anyhow::anyhow!("Unsupported sink type: {}", config.sink_type)),
        }
    }

    pub async fn send(&self, context: &Value) -> Result<(), anyhow::Error> {
        match self {
            AllSinks::Stdout(s) => s.send(context).await,
            AllSinks::Slack(s) => s.send(context).await,
            AllSinks::AlertManager(s) => s.send(context).await,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            AllSinks::Stdout(s) => s.name(),
            AllSinks::Slack(s) => s.name(),
            AllSinks::AlertManager(s) => s.name(),
        }
    }
}
*/ 