//! Clippy analysis runner.

use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;

use forgetest_core::results::{ClippyResult, CompilerDiagnostic, DiagnosticLevel, DiagnosticSpan};

use crate::sandbox::Sandbox;

/// Run clippy on the code in the sandbox.
pub async fn run_clippy(sandbox: &Sandbox) -> Result<ClippyResult> {
    let mut cmd = Command::new("cargo");
    cmd.arg("clippy")
        .arg("--message-format=json")
        .arg("--")
        .arg("-W")
        .arg("clippy::all")
        .current_dir(sandbox.work_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, val) in sandbox.build_env() {
        cmd.env(&key, &val);
    }

    let result = tokio::time::timeout(sandbox.timeout(), cmd.output())
        .await
        .context("clippy timed out")?
        .context("failed to run cargo clippy")?;

    let stdout = String::from_utf8_lossy(&result.stdout);
    let warnings = parse_clippy_output(&stdout);
    let warning_count = warnings.len() as u32;

    Ok(ClippyResult {
        warnings,
        warning_count,
    })
}

/// Parse clippy JSON output into diagnostics.
fn parse_clippy_output(output: &str) -> Vec<CompilerDiagnostic> {
    let mut warnings = Vec::new();

    for line in output.lines() {
        let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        if msg.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
            continue;
        }

        let Some(message) = msg.get("message") else {
            continue;
        };

        let level = message
            .get("level")
            .and_then(|l| l.as_str())
            .unwrap_or("note");

        if level != "warning" {
            continue;
        }

        // Only include clippy-specific warnings
        let code = message
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str());

        let is_clippy = code.is_some_and(|c| c.starts_with("clippy::"));
        if !is_clippy {
            continue;
        }

        let text = message
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        let spans = message
            .get("spans")
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|span| {
                        Some(DiagnosticSpan {
                            file: span.get("file_name")?.as_str()?.to_string(),
                            line_start: span.get("line_start")?.as_u64()? as u32,
                            line_end: span.get("line_end")?.as_u64()? as u32,
                            column_start: span.get("column_start")?.as_u64()? as u32,
                            column_end: span.get("column_end")?.as_u64()? as u32,
                            text: None,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        warnings.push(CompilerDiagnostic {
            level: DiagnosticLevel::Warning,
            message: text,
            code: code.map(|s| s.to_string()),
            spans,
        });
    }

    warnings
}

/// Check if the Rust toolchain and clippy are available.
pub async fn check_clippy_available() -> bool {
    Command::new("cargo")
        .arg("clippy")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}
