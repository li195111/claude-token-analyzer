use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::types::TokenUsage;

/// Pricing definition for a single model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_creation_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

/// Top-level pricing configuration (matches pricing.toml structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingConfig {
    pub models: Vec<ModelPricing>,
    pub defaults: DefaultPricing,
}

/// Fallback pricing for unknown models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultPricing {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_creation_per_mtok: f64,
    pub cache_read_per_mtok: f64,
}

/// Itemized cost breakdown from a single usage calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub input_cost: f64,
    pub output_cost: f64,
    pub cache_creation_cost: f64,
    pub cache_read_cost: f64,
    pub total_cost: f64,
}

/// Pricing lookup table with model resolution and cost calculation
#[derive(Debug, Clone)]
pub struct PricingTable {
    config: PricingConfig,
}

impl PricingTable {
    /// Parse a PricingTable from TOML content
    pub fn from_toml(content: &str) -> Result<Self> {
        let config: PricingConfig =
            toml::from_str(content).context("Failed to parse pricing TOML")?;
        Ok(Self { config })
    }

    /// Load the embedded pricing.toml compiled into the binary
    pub fn embedded() -> Self {
        let content = include_str!("../config/pricing.toml");
        Self::from_toml(content).expect("Embedded pricing.toml must be valid")
    }

    /// Load pricing from `$CTA_PRICING_PATH` if set, otherwise fall back to embedded.
    pub fn from_env_or_embedded() -> Result<Self> {
        match std::env::var("CTA_PRICING_PATH") {
            Ok(path) => {
                let content = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read pricing file: {}", path))?;
                Self::from_toml(&content)
                    .with_context(|| format!("Failed to parse pricing file: {}", path))
            }
            Err(_) => Ok(Self::embedded()),
        }
    }

    /// Look up pricing for a model by id or alias, falling back to defaults
    fn lookup(&self, model: &str) -> (&str, f64, f64, f64, f64) {
        for m in &self.config.models {
            if m.id == model || m.aliases.iter().any(|a| a == model) {
                return (
                    &m.id,
                    m.input_per_mtok,
                    m.output_per_mtok,
                    m.cache_creation_per_mtok,
                    m.cache_read_per_mtok,
                );
            }
        }
        let d = &self.config.defaults;
        (
            "default",
            d.input_per_mtok,
            d.output_per_mtok,
            d.cache_creation_per_mtok,
            d.cache_read_per_mtok,
        )
    }

    /// Calculate cost breakdown for a given model and token usage
    pub fn calculate_cost(&self, model: &str, usage: &TokenUsage) -> CostBreakdown {
        let (_, inp, out, cc, cr) = self.lookup(model);

        let input_cost = usage.input_tokens as f64 / 1_000_000.0 * inp;
        let output_cost = usage.output_tokens as f64 / 1_000_000.0 * out;
        let cache_creation_cost = usage.cache_creation_input_tokens as f64 / 1_000_000.0 * cc;
        let cache_read_cost = usage.cache_read_input_tokens as f64 / 1_000_000.0 * cr;
        let total_cost = input_cost + output_cost + cache_creation_cost + cache_read_cost;

        CostBreakdown {
            input_cost,
            output_cost,
            cache_creation_cost,
            cache_read_cost,
            total_cost,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::with_env_vars;

    #[test]
    fn test_embedded_pricing() {
        let table = PricingTable::embedded();
        assert!(
            !table.config.models.is_empty(),
            "Embedded pricing must contain at least one model"
        );
    }

    #[test]
    fn test_cost_calculation() {
        let table = PricingTable::embedded();
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
        };
        let cost = table.calculate_cost("claude-opus-4-6", &usage);

        // opus pricing: input=15, output=75, cache_creation=3.75, cache_read=0.75
        let epsilon = 0.001;
        assert!((cost.input_cost - 15.0).abs() < epsilon);
        assert!((cost.output_cost - 75.0).abs() < epsilon);
        assert!((cost.cache_creation_cost - 3.75).abs() < epsilon);
        assert!((cost.cache_read_cost - 0.75).abs() < epsilon);
        assert!((cost.total_cost - 94.5).abs() < epsilon);
    }

    #[test]
    fn test_model_alias_lookup() {
        let table = PricingTable::embedded();
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        // Alias for opus
        let cost_alias = table.calculate_cost("claude-opus-4-6-20250610", &usage);
        let cost_id = table.calculate_cost("claude-opus-4-6", &usage);
        assert!(
            (cost_alias.input_cost - cost_id.input_cost).abs() < 0.001,
            "Alias should resolve to same pricing as canonical id"
        );
    }

    #[test]
    fn test_default_fallback() {
        let table = PricingTable::embedded();
        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 1_000_000,
            cache_creation_input_tokens: 1_000_000,
            cache_read_input_tokens: 1_000_000,
        };

        // Unknown model should use defaults (same as sonnet pricing in this config)
        let cost = table.calculate_cost("claude-unknown-99", &usage);
        let epsilon = 0.001;
        assert!((cost.input_cost - 3.0).abs() < epsilon);
        assert!((cost.output_cost - 15.0).abs() < epsilon);
        assert!((cost.cache_creation_cost - 3.75).abs() < epsilon);
        assert!((cost.cache_read_cost - 0.30).abs() < epsilon);
    }

    #[test]
    fn test_from_env_or_embedded_uses_env() {
        use std::io::Write;

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            r#"
[[models]]
id = "test-model"
aliases = []
input_per_mtok = 99.0
output_per_mtok = 99.0
cache_creation_per_mtok = 99.0
cache_read_per_mtok = 99.0

[defaults]
input_per_mtok = 1.0
output_per_mtok = 1.0
cache_creation_per_mtok = 1.0
cache_read_per_mtok = 1.0
"#
        )
        .unwrap();

        let path = tmp.path().to_str().unwrap().to_string();
        let table = with_env_vars(&[("CTA_PRICING_PATH", Some(path.as_str()))], || {
            PricingTable::from_env_or_embedded().unwrap()
        });

        let usage = TokenUsage {
            input_tokens: 1_000_000,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let cost = table.calculate_cost("test-model", &usage);
        assert!(
            (cost.input_cost - 99.0).abs() < 0.001,
            "Should use env-specified pricing file, got {}",
            cost.input_cost
        );
    }

    #[test]
    fn test_from_env_or_embedded_fallback() {
        let table = with_env_vars(&[("CTA_PRICING_PATH", None)], || {
            PricingTable::from_env_or_embedded().unwrap()
        });
        assert!(
            !table.config.models.is_empty(),
            "Fallback to embedded should have models"
        );
    }

    #[test]
    fn test_from_env_invalid_path_errors() {
        let result = with_env_vars(
            &[("CTA_PRICING_PATH", Some("/nonexistent/pricing.toml"))],
            PricingTable::from_env_or_embedded,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("nonexistent"),
            "Error should mention the path: {}",
            err
        );
    }
}
