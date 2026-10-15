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
    /// Fail a capped attempt whose usage cannot be verified instead of
    /// recording it as unavailable. The benchmark profile sets this.
    #[serde(default)]
    pub require_enforceable_usage: bool,
    /// A run-level cost cap is active, so every attempt needs a known cost
    /// when `require_enforceable_usage` is set.
    #[serde(default)]
    pub run_cost_capped: bool,
}

impl Default for AgentLimits {
    fn default() -> Self {
        Self {
            timeout_secs: 900,
            max_output_bytes: 4 * 1024 * 1024,
            max_retries: 0,
            max_tokens: None,
            max_cost_usd: None,
            require_enforceable_usage: false,
            run_cost_capped: false,
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
///
/// In records that carry `cost_source`, `input_tokens` is uncached input,
/// `cached_tokens` is cache reads, and `output_tokens` includes reasoning.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_tokens: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_write_tokens: u64,
    /// Informational subset of `output_tokens`; never priced separately.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub reasoning_tokens: u64,
    pub estimated_cost_usd: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_source: Option<CostSource>,
}

impl AgentUsage {
    /// Usage of an attempt whose consumption is unknown.
    pub fn unavailable() -> Self {
        Self {
            cost_source: Some(CostSource::Unavailable),
            ..Self::default()
        }
    }

    /// Tokens counted against a token cap: uncached input, cache reads,
    /// cache writes, and output.
    pub fn budget_tokens(&self) -> u64 {
        self.input_tokens
            .saturating_add(self.cached_tokens)
            .saturating_add(self.cache_write_tokens)
            .saturating_add(self.output_tokens)
    }

    /// Whether `estimated_cost_usd` counts toward aggregate cost totals.
    /// Legacy records without a cost source keep counting, as before.
    pub fn cost_available(&self) -> bool {
        self.cost_source != Some(CostSource::Unavailable)
    }

    /// Whether the cost is known well enough to enforce a cap: reported by
    /// the vendor or priced, and a finite, non-negative amount.
    pub fn cost_known(&self) -> bool {
        matches!(
            self.cost_source,
            Some(CostSource::Vendor | CostSource::PricingTable)
        ) && self.estimated_cost_usd.is_finite()
            && self.estimated_cost_usd >= 0.0
    }
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// Origin of a recorded agent cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CostSource {
    /// Reported by the agent vendor in its event stream.
    Vendor,
    /// Computed from reported tokens and the policy pricing table.
    PricingTable,
    /// Unknown; excluded from cost totals and never treated as zero.
    Unavailable,
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
    /// A cap was set but the agent did not report the usage needed to check it.
    BudgetUnverifiable,
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
