//! Agent Result Structures
//! 
//! Defines the output format for agent investigations.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result from an agent investigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Summary of the investigation findings
    pub summary: String,
    
    /// Detailed findings from the investigation
    pub findings: Vec<Finding>,
    
    /// Root cause analysis (if determined)
    pub root_cause: Option<String>,
    
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    
    /// Actions taken during investigation
    pub actions_taken: Vec<ActionTaken>,
    
    /// Recommendations for resolution
    pub recommendations: Vec<Recommendation>,
    
    /// Whether the agent believes it can auto-fix
    pub can_auto_fix: bool,
    
    /// Proposed fix command (if can_auto_fix is true)
    pub fix_command: Option<String>,
    
    /// Context for escalation if manual intervention needed
    pub escalation_notes: Option<String>,
    
    /// Raw conversation history (for debugging)
    pub conversation: Vec<ConversationTurn>,
}

/// A specific finding from the investigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: String,
    pub description: String,
    pub severity: FindingSeverity,
    pub evidence: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

/// Record of an action taken by the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTaken {
    pub tool: String,
    pub command: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub success: bool,
    pub output_summary: String,
}

/// Recommendation for resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub priority: u8, // 1 = highest priority
    pub action: String,
    pub rationale: String,
    pub risk_level: RiskLevel,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// A turn in the agent conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for AgentResult {
    fn default() -> Self {
        Self {
            summary: String::new(),
            findings: Vec::new(),
            root_cause: None,
            confidence: 0.0,
            actions_taken: Vec::new(),
            recommendations: Vec::new(),
            can_auto_fix: false,
            fix_command: None,
            escalation_notes: None,
            conversation: Vec::new(),
        }
    }
}

impl AgentResult {
    /// Create a new agent result
    pub fn new(summary: String) -> Self {
        Self {
            summary,
            ..Default::default()
        }
    }
    
    /// Add a finding to the result
    pub fn add_finding(&mut self, finding: Finding) {
        self.findings.push(finding);
    }
    
    /// Add an action taken
    pub fn add_action(&mut self, action: ActionTaken) {
        self.actions_taken.push(action);
    }
    
    /// Add a recommendation
    pub fn add_recommendation(&mut self, recommendation: Recommendation) {
        self.recommendations.push(recommendation);
        // Keep recommendations sorted by priority
        self.recommendations.sort_by_key(|r| r.priority);
    }
    
    /// Format as a human-readable report
    pub fn format_report(&self) -> String {
        let mut report = String::new();
        
        // Summary
        report.push_str("# Investigation Summary\n\n");
        report.push_str(&self.summary);
        report.push_str("\n\n");
        
        // Confidence
        report.push_str(&format!("**Confidence Level**: {:.0}%\n\n", self.confidence * 100.0));
        
        // Root Cause
        if let Some(root_cause) = &self.root_cause {
            report.push_str("## Root Cause\n\n");
            report.push_str(root_cause);
            report.push_str("\n\n");
        }
        
        // Findings
        if !self.findings.is_empty() {
            report.push_str("## Key Findings\n\n");
            for finding in &self.findings {
                report.push_str(&format!("- **{}** ({}): {}\n", 
                    finding.category, 
                    format!("{:?}", finding.severity).to_lowercase(),
                    finding.description
                ));
            }
            report.push_str("\n");
        }
        
        // Actions Taken
        if !self.actions_taken.is_empty() {
            report.push_str("## Investigation Steps\n\n");
            for action in &self.actions_taken {
                let status = if action.success { "✓" } else { "✗" };
                report.push_str(&format!("{} `{}`: {}\n", 
                    status,
                    action.command,
                    action.output_summary
                ));
            }
            report.push_str("\n");
        }
        
        // Recommendations
        if !self.recommendations.is_empty() {
            report.push_str("## Recommendations\n\n");
            for rec in &self.recommendations {
                let approval = if rec.requires_approval { " (requires approval)" } else { "" };
                report.push_str(&format!("{}. **{}** - {}{}\n", 
                    rec.priority,
                    rec.action,
                    rec.rationale,
                    approval
                ));
            }
            report.push_str("\n");
        }
        
        // Auto-fix
        if self.can_auto_fix {
            report.push_str("## Automated Resolution Available\n\n");
            if let Some(fix) = &self.fix_command {
                report.push_str(&format!("Proposed fix: `{}`\n\n", fix));
            }
        }
        
        // Escalation
        if let Some(notes) = &self.escalation_notes {
            report.push_str("## Escalation Context\n\n");
            report.push_str(notes);
            report.push_str("\n");
        }
        
        report
    }
} 