use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowState {
    Pending,
    Running,
    Succeeded,
    Failed,
}

impl fmt::Display for WorkflowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkflowState::Pending => write!(f, "Pending"),
            WorkflowState::Running => write!(f, "Running"),
            WorkflowState::Succeeded => write!(f, "Succeeded"),
            WorkflowState::Failed => write!(f, "Failed"),
        }
    }
}

impl From<&str> for WorkflowState {
    fn from(s: &str) -> Self {
        match s {
            "Pending" => WorkflowState::Pending,
            "Running" => WorkflowState::Running,
            "Succeeded" => WorkflowState::Succeeded,
            "Failed" => WorkflowState::Failed,
            _ => WorkflowState::Pending,
        }
    }
} 