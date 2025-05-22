use k8s_openapi::api::batch::v1::Job;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskJob {
    pub name: String,
    pub namespace: String,
    pub status: Option<JobStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobStatus {
    pub active: Option<i32>,
    pub succeeded: Option<i32>,
    pub failed: Option<i32>,
    pub conditions: Option<Vec<JobCondition>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JobCondition {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub last_probe_time: Option<String>,
    pub last_transition_time: Option<String>,
} 