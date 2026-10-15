//! Operator-supplied model prices for agents that report tokens but no cost.
//!
//! No prices ship with forgetest. The table is embedded inline in the
//! execution policy so its digest covers the exact prices used.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::agent::AgentUsage;

/// Supported `pricing.toml` schema version.
pub const PRICING_SCHEMA_VERSION: u32 = 1;

/// Exact-model prices in USD per million tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PricingTable {
    pub schema_version: u32,
    /// Date the operator copied the prices, as `YYYY-MM-DD`.
    pub retrieved_on: String,
    pub models: Vec<ModelPricing>,
}

/// Prices for one exact model ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelPricing {
    /// Exact model ID. Aliases, prefixes, and case variants never match.
    pub model: String,
    pub input_usd_per_mtok: f64,
    pub cache_read_usd_per_mtok: f64,
    pub cache_write_usd_per_mtok: f64,
    pub output_usd_per_mtok: f64,
}

impl PricingTable {
    /// Parse and validate a `pricing.toml` document. Entries are sorted by
    /// model ID so reordering a file does not change the policy digest.
    pub fn parse(content: &str) -> Result<Self> {
        let mut table: Self = toml::from_str(content).context("failed to parse pricing table")?;
        table
            .models
            .sort_by(|left, right| left.model.cmp(&right.model));
        table.validate()?;
        // Checked only when a file is read, so stored reports stay deterministic.
        let today = chrono::Local::now()
            .date_naive()
            .max(chrono::Utc::now().date_naive());
        let retrieved_on = chrono::NaiveDate::parse_from_str(&table.retrieved_on, "%Y-%m-%d")
            .context("pricing table retrieved_on must be a quoted YYYY-MM-DD date")?;
        anyhow::ensure!(
            retrieved_on <= today,
            "pricing table retrieved_on {} is later than today",
            table.retrieved_on
        );
        Ok(table)
    }

    /// Load a pricing file. Errors never include the host path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).context("failed to read pricing table")?;
        Self::parse(&content)
    }

    /// Reject anything that could price a trial ambiguously or impossibly.
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.schema_version == PRICING_SCHEMA_VERSION,
            "unsupported pricing table schema version: {}",
            self.schema_version
        );
        anyhow::ensure!(
            chrono::NaiveDate::parse_from_str(&self.retrieved_on, "%Y-%m-%d")
                .is_ok_and(|date| { date.format("%Y-%m-%d").to_string() == self.retrieved_on }),
            "pricing table retrieved_on must be a quoted YYYY-MM-DD date"
        );
        anyhow::ensure!(!self.models.is_empty(), "pricing table contains no models");
        let mut seen = HashSet::new();
        for entry in &self.models {
            anyhow::ensure!(
                !entry.model.is_empty()
                    && !entry
                        .model
                        .chars()
                        .any(|character| character.is_whitespace() || character.is_control()),
                "pricing table model IDs must be non-empty without whitespace"
            );
            anyhow::ensure!(
                seen.insert(entry.model.as_str()),
                "duplicate pricing table model: {}",
                entry.model
            );
            for (name, value) in [
                ("input_usd_per_mtok", entry.input_usd_per_mtok),
                ("cache_read_usd_per_mtok", entry.cache_read_usd_per_mtok),
                ("cache_write_usd_per_mtok", entry.cache_write_usd_per_mtok),
                ("output_usd_per_mtok", entry.output_usd_per_mtok),
            ] {
                anyhow::ensure!(
                    value.is_finite() && !value.is_sign_negative(),
                    "pricing table model '{}' has an invalid {name}: prices must be finite and non-negative",
                    entry.model
                );
            }
        }
        Ok(())
    }

    /// Prices for an exact model ID.
    pub fn price(&self, model: &str) -> Option<&ModelPricing> {
        self.models.iter().find(|entry| entry.model == model)
    }

    /// Cost of normalized usage, or `None` when the model is not priced.
    /// Reasoning tokens are part of output and are not priced separately.
    pub fn cost(&self, model: &str, usage: &AgentUsage) -> Option<f64> {
        let price = self.price(model)?;
        Some(
            (usage.input_tokens as f64 * price.input_usd_per_mtok
                + usage.cached_tokens as f64 * price.cache_read_usd_per_mtok
                + usage.cache_write_tokens as f64 * price.cache_write_usd_per_mtok
                + usage.output_tokens as f64 * price.output_usd_per_mtok)
                / 1_000_000.0,
        )
    }
}
