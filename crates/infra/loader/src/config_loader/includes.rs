//! Include-file loading with root-only `settings:` enforcement.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use systemprompt_models::services::ServicesConfig;

use crate::error::{ConfigLoadError, ConfigLoadResult};

use super::merge::merge_into;
use super::types::IncludeResolveCtx;

pub(super) fn resolve_includes_recursively(
    base_path: &Path,
    include_path: &str,
    referrer: &Path,
    ctx: &mut IncludeResolveCtx<'_>,
) -> ConfigLoadResult<()> {
    let referrer_dir = referrer.parent().unwrap_or(base_path);
    let full_path = referrer_dir.join(include_path);

    if !full_path.exists() {
        return Err(ConfigLoadError::IncludeNotFound {
            include: full_path,
            referrer: referrer.to_path_buf(),
        });
    }

    let canonical = fs::canonicalize(&full_path).map_err(|e| ConfigLoadError::Io {
        path: full_path.clone(),
        source: e,
    })?;

    if ctx.visited.contains(&canonical) {
        let mut chain: Vec<String> = ctx.chain.iter().map(|p| p.display().to_string()).collect();
        chain.push(canonical.display().to_string());
        return Err(ConfigLoadError::IncludeCycle {
            chain: chain.join(" -> "),
        });
    }
    ctx.visited.insert(canonical.clone());

    let content = fs::read_to_string(&canonical).map_err(|e| ConfigLoadError::Io {
        path: canonical.clone(),
        source: e,
    })?;

    reject_settings_at_include(&content, &canonical)?;

    let mut included: ServicesConfig =
        serde_yaml::from_str(&content).map_err(|e| ConfigLoadError::Yaml {
            path: canonical.clone(),
            source: e,
        })?;

    let nested_includes = std::mem::take(&mut included.includes);

    ctx.chain.push(canonical.clone());
    for nested in &nested_includes {
        resolve_includes_recursively(base_path, nested, &canonical, ctx)?;
    }
    ctx.chain.pop();

    let file_dir = canonical.parent().unwrap_or(base_path).to_path_buf();
    super::merge::resolve_system_prompt_includes(&file_dir, &mut included)?;
    super::merge::resolve_skill_instruction_includes(&file_dir, &mut included)?;
    merge_into(ctx.merged, included)?;

    Ok(())
}

// Why: Sniff the YAML for a top-level `settings:` key before deserializing.
//
// Settings are only meaningful at the root config; an include that sets
// them is almost certainly an operator error (the values would otherwise
// be silently ignored). Reject explicitly so the misconfiguration shows
// up at startup.
fn reject_settings_at_include(content: &str, path: &Path) -> ConfigLoadResult<()> {
    let value: serde_yaml::Value = match serde_yaml::from_str(content) {
        Ok(v) => v,
        // Why: Defer the error to the typed deserialize path so callers get
        // the structured Yaml error variant with the proper context.
        Err(_) => return Ok(()),
    };
    if let serde_yaml::Value::Mapping(map) = value
        && map.contains_key(serde_yaml::Value::String("settings".into()))
    {
        return Err(ConfigLoadError::IncludeMustNotSetGlobalSettings {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}
