//! The `forgetest redact` command.

use std::path::PathBuf;

use anyhow::Result;
use forgetest_core::repository_report::RepositoryReport;
use forgetest_report::redaction::{redact_repository_report, RedactionOptions};

pub fn execute(input: PathBuf, output: PathBuf, format: String) -> Result<()> {
    crate::commands::run::parse_formats(&format)?;
    let report = RepositoryReport::load_json(&input)?;
    crate::commands::demo::ensure_fresh_evidence_directory(&output)?;
    let mut path_replacements = Vec::new();
    if let Some(parent) = input.parent() {
        path_replacements.push((parent.to_path_buf(), "$INPUT".into()));
    }
    if let Ok(current) = std::env::current_dir() {
        path_replacements.push((current, "$WORKSPACE".into()));
    }
    if let Some(home) = std::env::var_os("HOME") {
        path_replacements.push((PathBuf::from(home), "$HOME".into()));
    }
    let secret_values = [
        "OPENAI_API_KEY",
        "ANTHROPIC_API_KEY",
        "FORGETEST_OPENAI_KEY",
        "FORGETEST_ANTHROPIC_KEY",
        "GITHUB_TOKEN",
        "GH_TOKEN",
    ]
    .into_iter()
    .filter_map(|name| std::env::var(name).ok())
    .collect();
    let public = redact_repository_report(
        &report,
        &RedactionOptions {
            path_replacements,
            secret_values,
        },
    )?;
    crate::commands::demo::write_repository_outputs(&public, &output, &format)?;
    eprintln!(
        "Public artifacts written to {} ({} replacement(s), rules v{})",
        output.display(),
        public.redaction.replacements,
        public
            .redaction
            .rules_version
            .as_deref()
            .unwrap_or("unknown")
    );
    Ok(())
}
