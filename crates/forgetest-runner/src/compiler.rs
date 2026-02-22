//! Compilation runner for sandboxed Cargo projects.

use std::process::Stdio;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::process::Command;

use forgetest_core::results::{CompilationResult, CompilerDiagnostic, DiagnosticLevel, DiagnosticSpan};

use crate::sandbox::Sandbox;

/// Compile the code in a sandbox.
pub async fn compile(sandbox: &Sandbox) -> Result<CompilationResult> {
    let start = Instant::now();

    let mut cmd = Command::new("cargo");
    cmd.arg("build")
        .arg("--message-format=json")
        .current_dir(sandbox.work_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, val) in sandbox.build_env() {
        cmd.env(&key, &val);
    }

    let result = tokio::time::timeout(sandbox.timeout(), cmd.output())
        .await
        .context("compilation timed out")?
        .context("failed to run cargo build")?;

    let duration_ms = start.elapsed().as_millis() as u64;
    let stdout = String::from_utf8_lossy(&result.stdout);

    let (errors, warnings) = parse_cargo_json_output(&stdout);

    Ok(CompilationResult {
        success: result.status.success(),
        errors,
        warnings,
        duration_ms,
    })
}

/// Parse cargo's JSON output into diagnostics.
fn parse_cargo_json_output(output: &str) -> (Vec<CompilerDiagnostic>, Vec<CompilerDiagnostic>) {
    let mut errors = Vec::new();
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

        let level_str = message
            .get("level")
            .and_then(|l| l.as_str())
            .unwrap_or("note");

        let level = match level_str {
            "error" => DiagnosticLevel::Error,
            "warning" => DiagnosticLevel::Warning,
            "note" => DiagnosticLevel::Note,
            "help" => DiagnosticLevel::Help,
            _ => DiagnosticLevel::Note,
        };

        let text = message
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        let code = message
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string());

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
                            text: span
                                .get("text")
                                .and_then(|t| t.as_array())
                                .and_then(|a| a.first())
                                .and_then(|t| t.get("text"))
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string()),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let diagnostic = CompilerDiagnostic {
            level,
            message: text,
            code,
            spans,
        };

        match level {
            DiagnosticLevel::Error => errors.push(diagnostic),
            DiagnosticLevel::Warning => warnings.push(diagnostic),
            _ => {} // Skip notes and help for now
        }
    }

    (errors, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::Sandbox;
    use forgetest_core::model::Language;
    use std::time::Duration;

    #[tokio::test]
    async fn compile_valid_code() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            Sandbox::new(Language::Rust, Duration::from_secs(120), target.path()).unwrap();
        sandbox
            .write_source("pub fn add(a: i32, b: i32) -> i32 { a + b }")
            .unwrap();

        let result = compile(&sandbox).await.unwrap();
        assert!(result.success, "compilation should succeed");
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn compile_invalid_code() {
        let target = tempfile::tempdir().unwrap();
        let sandbox =
            Sandbox::new(Language::Rust, Duration::from_secs(120), target.path()).unwrap();
        sandbox
            .write_source("pub fn bad() -> i32 { \"not an int\" }")
            .unwrap();

        let result = compile(&sandbox).await.unwrap();
        assert!(!result.success, "compilation should fail");
        assert!(!result.errors.is_empty());
    }
}
