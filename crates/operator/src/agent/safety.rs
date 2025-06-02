//! Safety Module for Agent Operations
//! 
//! Provides validation and safety checks for agent actions.

use anyhow::Result;
use regex::Regex;
use std::collections::HashSet;

/// Safety configuration for agent operations
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    /// Commands that require approval
    pub approval_required: HashSet<String>,
    
    /// Regex patterns for dangerous commands
    pub dangerous_patterns: Vec<Regex>,
    
    /// Maximum command length
    pub max_command_length: usize,
    
    /// Allow destructive operations
    pub allow_destructive: bool,
    
    /// Resource limits
    pub max_iterations: u32,
    pub max_execution_time: std::time::Duration,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        let mut approval_required = HashSet::new();
        approval_required.insert("delete".to_string());
        approval_required.insert("scale".to_string());
        approval_required.insert("patch".to_string());
        approval_required.insert("replace".to_string());
        approval_required.insert("drain".to_string());
        approval_required.insert("cordon".to_string());
        
        let dangerous_patterns = vec![
            Regex::new(r"rm\s+-rf").unwrap(),
            Regex::new(r"kubectl\s+delete\s+namespace").unwrap(),
            Regex::new(r"kubectl\s+delete\s+--all").unwrap(),
            Regex::new(r":\s*\(\s*\)").unwrap(), // Fork bomb
            Regex::new(r">\s*/dev/sd").unwrap(), // Disk overwrite
        ];
        
        Self {
            approval_required,
            dangerous_patterns,
            max_command_length: 1000,
            allow_destructive: false,
            max_iterations: 20,
            max_execution_time: std::time::Duration::from_secs(600), // 10 minutes
        }
    }
}

/// Safety validator for agent operations
#[derive(Clone)]
pub struct SafetyValidator {
    config: SafetyConfig,
}

impl SafetyValidator {
    pub fn new(config: SafetyConfig) -> Self {
        Self { config }
    }
    
    /// Check if a command requires approval
    pub fn requires_approval(&self, command: &str) -> bool {
        // Check if any approval-required verb is in the command
        for verb in &self.config.approval_required {
            if command.contains(verb) {
                return true;
            }
        }
        false
    }
    
    /// Validate a command for safety
    pub fn validate_command(&self, command: &str) -> Result<()> {
        // Check command length
        if command.len() > self.config.max_command_length {
            return Err(anyhow::anyhow!(
                "Command exceeds maximum length of {} characters",
                self.config.max_command_length
            ));
        }
        
        // Check dangerous patterns
        for pattern in &self.config.dangerous_patterns {
            if pattern.is_match(command) {
                return Err(anyhow::anyhow!(
                    "Command matches dangerous pattern: {}",
                    pattern.as_str()
                ));
            }
        }
        
        // Check for destructive operations
        if !self.config.allow_destructive && self.is_destructive(command) {
            return Err(anyhow::anyhow!(
                "Destructive operations are not allowed"
            ));
        }
        
        Ok(())
    }
    
    /// Check if a command is destructive
    fn is_destructive(&self, command: &str) -> bool {
        let destructive_verbs = ["delete", "remove", "destroy", "drop", "truncate"];
        let lower_command = command.to_lowercase();
        
        destructive_verbs.iter().any(|verb| lower_command.contains(verb))
    }
    
    /// Sanitize a command by removing potentially dangerous elements
    pub fn sanitize_command(&self, command: &str) -> String {
        let mut sanitized = command.to_string();
        
        // Remove shell metacharacters
        let metacharacters = ['$', '`', '\\', '"', '\'', ';', '&', '|', '>', '<'];
        for ch in metacharacters {
            sanitized = sanitized.replace(ch, "");
        }
        
        // Remove multiple spaces
        let re = Regex::new(r"\s+").unwrap();
        sanitized = re.replace_all(&sanitized, " ").to_string();
        
        sanitized.trim().to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
} 