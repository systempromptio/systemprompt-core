//! Reconcile `services/ai/config.yaml` with the provider chosen during setup.
//!
//! `admin setup` generates the profile and secrets, but the AI
//! service layer reads `services/ai/config.yaml` for its `default_provider` and
//! per-provider `enabled` flags. When the operator picks a default provider,
//! align that file: make the choice the default and disable the standard
//! providers whose keys were not supplied. Custom providers (e.g. `minimax`)
//! and every other field are left untouched. An absent file is a no-op —
//! minimal instances need not ship one.

use std::path::Path;

use anyhow::{Context, Result};
use serde_yaml::Value;
use systemprompt_identifiers::ProviderId;
use systemprompt_logging::CliService;

use super::secrets::SecretsData;
use crate::CliConfig;
use crate::commands::admin::config::config_section::{read_yaml_file, write_yaml_file};

const STANDARD_PROVIDERS: [&str; 3] = ["gemini", "anthropic", "openai"];

pub(super) fn reconcile(
    project_root: &Path,
    primary: &ProviderId,
    secrets: &SecretsData,
    config: &CliConfig,
) -> Result<()> {
    let path = project_root.join("services").join("ai").join("config.yaml");
    if !path.exists() {
        if !config.is_json_output() {
            CliService::info(&format!(
                "No {} — skipping AI default-provider reconcile",
                path.display()
            ));
        }
        return Ok(());
    }

    let mut doc = read_yaml_file(&path)?;
    apply_ai_defaults(&mut doc, primary.as_str(), &secrets.present_providers())?;
    write_yaml_file(&path, &doc)?;

    if !config.is_json_output() {
        CliService::success(&format!(
            "Set AI default provider to '{}' in {}",
            primary.as_str(),
            path.display()
        ));
    }
    Ok(())
}

/// Custom providers (e.g. `minimax`) and every field other than the standard
/// providers' `enabled` flag are left untouched.
pub fn apply_ai_defaults(doc: &mut Value, default_provider: &str, present: &[&str]) -> Result<()> {
    let ai = doc
        .get_mut("ai")
        .and_then(Value::as_mapping_mut)
        .context("services/ai/config.yaml has no 'ai' mapping")?;

    ai.insert(
        Value::String("default_provider".to_owned()),
        Value::String(default_provider.to_owned()),
    );

    if let Some(providers) = ai
        .get_mut(Value::String("providers".to_owned()))
        .and_then(Value::as_mapping_mut)
    {
        for name in STANDARD_PROVIDERS {
            if let Some(block) = providers
                .get_mut(Value::String(name.to_owned()))
                .and_then(Value::as_mapping_mut)
            {
                block.insert(
                    Value::String("enabled".to_owned()),
                    Value::Bool(present.contains(&name)),
                );
            }
        }
    }

    Ok(())
}
