//! Language-neutral interfaces for repository coding agents and graders.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::repository_report::{GraderCheckKind, GraderOutcome};

/// Stable identity for one evaluated agent configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub adapter: String,
    pub adapter_version: String,
    #[serde(default)]
    pub executable_sha256: Option<String>,
    pub model: String,
    pub configuration_digest: String,
}

impl AgentIdentity {
    /// Exact human-readable key used in aggregate maps.
    pub fn key(&self) -> String {
        format!(
            "{}/{}@{}#{}",
            self.adapter, self.model, self.adapter_version, self.configuration_digest
        )
    }
}

/// Hard limits applied to one agent trial.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentLimits {
    pub timeout_secs: u64,
    pub max_output_bytes: usize,
    pub max_retries: u32,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub max_cost_usd: Option<f64>,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            timeout_secs: 900,
            max_output_bytes: 4 * 1024 * 1024,
            max_retries: 0,
            max_tokens: None,
            max_cost_usd: None,
        }
    }
}

/// Input passed to an agent adapter.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    pub trial_id: Uuid,
    pub task_id: String,
    pub prompt: String,
    pub workspace: PathBuf,
    pub limits: AgentLimits,
}

/// Normalized event emitted by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub sequence: u64,
    pub timestamp: DateTime<Utc>,
    pub kind: AgentEventKind,
    pub message: String,
    #[serde(default)]
    pub raw: Option<serde_json::Value>,
}

/// Portable categories for vendor-specific agent events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentEventKind {
    Started,
    Message,
    ToolCall,
    ToolResult,
    Usage,
    Warning,
    Error,
    Completed,
    Unknown,
}

/// Token and cost observations reported by an adapter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    pub estimated_cost_usd: f64,
}

/// Why an agent process stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTerminationReason {
    Completed,
    ExitNonZero,
    Timeout,
    OutputLimit,
    BudgetExceeded,
    Cancelled,
}

/// Result returned by an agent adapter before independent grading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOutcome {
    pub identity: AgentIdentity,
    pub termination: AgentTerminationReason,
    #[serde(default)]
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    #[serde(default)]
    pub usage: AgentUsage,
    #[serde(default)]
    pub events: Vec<AgentEvent>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Receives trace events as they occur.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &AgentEvent) -> Result<()>;
}

/// Executes one coding-agent trial against a writable workspace.
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    fn identity(&self) -> &AgentIdentity;
    async fn execute(&self, request: &AgentRequest, events: &dyn EventSink)
        -> Result<AgentOutcome>;
}

/// Environment boundary used to execute an agent.
#[async_trait]
pub trait WorkspaceEnvironment: Send + Sync {
    fn identity(&self) -> String;
    async fn execute(
        &self,
        agent: &dyn AgentExecutor,
        request: &AgentRequest,
        events: &dyn EventSink,
    ) -> Result<AgentOutcome>;
}

/// Request passed to an independent repository grader.
#[derive(Debug, Clone)]
pub struct GradeRequest {
    pub trial_id: Uuid,
    pub workspace: PathBuf,
    pub checks: Vec<GradeCheckRequest>,
    pub timeout: Duration,
}

/// One deterministic command the independent grader must execute.
#[derive(Debug, Clone)]
pub struct GradeCheckRequest {
    pub name: String,
    pub kind: GraderCheckKind,
    pub command: Vec<String>,
}

/// Independently grades a patched repository workspace.
#[async_trait]
pub trait Grader: Send + Sync {
    fn identity(&self) -> String;
    async fn grade(&self, request: &GradeRequest) -> Result<GraderOutcome>;
}
