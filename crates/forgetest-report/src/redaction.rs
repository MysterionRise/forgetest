//! Publication-safe sanitization for repository evidence reports.

use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::Utc;
use forgetest_core::agent::AgentEventKind;
use forgetest_core::repository_report::RepositoryReport;
use regex::Regex;

/// Current deterministic sanitizer ruleset.
pub const REDACTION_RULES_VERSION: &str = "1";

/// Caller-provided path aliases and known secret values.
#[derive(Debug, Clone, Default)]
pub struct RedactionOptions {
    pub path_replacements: Vec<(PathBuf, String)>,
    pub secret_values: Vec<String>,
}

/// Produce a public report while retaining metrics and content identities.
pub fn redact_repository_report(
    report: &RepositoryReport,
    options: &RedactionOptions,
) -> Result<RepositoryReport> {
    let mut value = serde_json::to_value(report).context("failed to serialize report")?;
    let mut redactor = Redactor::new(options)?;
    redactor.redact_value(&mut value);
    let mut redacted: RepositoryReport =
        serde_json::from_value(value).context("failed to rebuild redacted report")?;
    for trial in &mut redacted.trials {
        redactor.replacements += trial.artifacts.len() as u64;
        trial.artifacts.clear();
        let event_count = trial.events.len();
        trial.events.retain(|event| {
            !matches!(
                event.kind,
                AgentEventKind::Message | AgentEventKind::Unknown
            )
        });
        redactor.replacements += event_count.saturating_sub(trial.events.len()) as u64;
        for event in &mut trial.events {
            if event.raw.take().is_some() {
                redactor.replacements += 1;
            }
            let label = public_event_label(event.kind);
            if event.message != label {
                event.message = label.into();
                redactor.replacements += 1;
            }
        }
    }
    redacted.redaction.redacted = true;
    redacted.redaction.redacted_at = Some(Utc::now());
    redacted.redaction.rules_version = Some(REDACTION_RULES_VERSION.into());
    redacted.redaction.replacements = redactor.replacements;
    Ok(redacted)
}

fn public_event_label(kind: AgentEventKind) -> &'static str {
    match kind {
        AgentEventKind::Started => "started",
        AgentEventKind::Message => "message",
        AgentEventKind::ToolCall => "tool_call",
        AgentEventKind::ToolResult => "tool_result",
        AgentEventKind::Usage => "usage",
        AgentEventKind::Warning => "warning",
        AgentEventKind::Error => "error",
        AgentEventKind::Completed => "completed",
        AgentEventKind::Unknown => "unknown",
    }
}

struct Redactor {
    path_replacements: Vec<(String, String)>,
    secret_values: Vec<String>,
    patterns: Vec<Regex>,
    replacements: u64,
}

impl Redactor {
    fn new(options: &RedactionOptions) -> Result<Self> {
        let mut path_replacements: Vec<_> = options
            .path_replacements
            .iter()
            .filter_map(|(path, replacement)| {
                let value = path.to_string_lossy().to_string();
                (!value.is_empty()).then(|| (value, replacement.clone()))
            })
            .collect();
        path_replacements.sort_by(|left, right| right.0.len().cmp(&left.0.len()));
        let mut secret_values: Vec<_> = options
            .secret_values
            .iter()
            .filter(|secret| secret.len() >= 4)
            .cloned()
            .collect();
        secret_values.sort_by_key(|right| std::cmp::Reverse(right.len()));
        let patterns = [
            r"(?i)\b(?:sk|sk-ant)-[a-z0-9_-]{12,}\b",
            r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,})\b",
            r"\bAKIA[0-9A-Z]{16}\b",
            r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{16,}",
            r#"(?i)\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|password|secret)\s*[=:]\s*["']?[A-Za-z0-9._~+/=-]{8,}["']?"#,
        ]
        .into_iter()
        .map(Regex::new)
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to compile redaction rules")?;
        Ok(Self {
            path_replacements,
            secret_values,
            patterns,
            replacements: 0,
        })
    }

    fn redact_value(&mut self, value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                let private_keys: Vec<_> = object
                    .keys()
                    .filter(|key| is_private_reasoning_key(key))
                    .cloned()
                    .collect();
                for key in private_keys {
                    object.remove(&key);
                    self.replacements += 1;
                }
                for value in object.values_mut() {
                    self.redact_value(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    self.redact_value(value);
                }
            }
            serde_json::Value::String(value) => self.redact_string(value),
            serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            }
        }
    }

    fn redact_string(&mut self, value: &mut String) {
        for (path, replacement) in &self.path_replacements {
            let matches = value.matches(path).count() as u64;
            if matches > 0 {
                *value = value.replace(path, replacement);
                self.replacements += matches;
            }
        }
        for secret in &self.secret_values {
            let matches = value.matches(secret).count() as u64;
            if matches > 0 {
                *value = value.replace(secret, "[REDACTED]");
                self.replacements += matches;
            }
        }
        for pattern in &self.patterns {
            let matches = pattern.find_iter(value).count() as u64;
            if matches > 0 {
                *value = pattern.replace_all(value, "[REDACTED]").into_owned();
                self.replacements += matches;
            }
        }
    }
}

fn is_private_reasoning_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "reasoning"
            | "thinking"
            | "chain_of_thought"
            | "chainofthought"
            | "encrypted_content"
            | "private_reasoning"
    )
}
