use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;
use systemprompt_logging::CliService;
use systemprompt_models::profile::RateLimitsConfig;
use systemprompt_models::ProfileBootstrap;

use super::DiffArgs;
use crate::cli_settings::OutputFormat;
use crate::shared::{render_result, CommandResult};
use crate::CliConfig;

use super::super::types::{DiffEntry, DiffOutput};

pub fn execute_diff(args: &DiffArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let current = &profile.rate_limits;

    let (compare_with, source) = if args.defaults {
        (RateLimitsConfig::default(), "defaults".to_string())
    } else if let Some(file_path) = &args.file {
        let content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read file: {}", file_path))?;

        let is_json = Path::new(file_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));

        let limits: RateLimitsConfig = if is_json {
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse JSON from: {}", file_path))?
        } else {
            serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse YAML from: {}", file_path))?
        };

        (limits, file_path.clone())
    } else {
        bail!("Must specify --defaults or --file");
    };

    let mut differences: Vec<DiffEntry> = Vec::new();

    add_diff_if_different(
        &mut differences,
        "disabled",
        &current.disabled,
        &compare_with.disabled,
    );
    add_diff_if_different(
        &mut differences,
        "oauth_public_per_second",
        &current.oauth_public_per_second,
        &compare_with.oauth_public_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "oauth_auth_per_second",
        &current.oauth_auth_per_second,
        &compare_with.oauth_auth_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "contexts_per_second",
        &current.contexts_per_second,
        &compare_with.contexts_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "tasks_per_second",
        &current.tasks_per_second,
        &compare_with.tasks_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "artifacts_per_second",
        &current.artifacts_per_second,
        &compare_with.artifacts_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "agent_registry_per_second",
        &current.agent_registry_per_second,
        &compare_with.agent_registry_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "agents_per_second",
        &current.agents_per_second,
        &compare_with.agents_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "mcp_registry_per_second",
        &current.mcp_registry_per_second,
        &compare_with.mcp_registry_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "mcp_per_second",
        &current.mcp_per_second,
        &compare_with.mcp_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "stream_per_second",
        &current.stream_per_second,
        &compare_with.stream_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "content_per_second",
        &current.content_per_second,
        &compare_with.content_per_second,
    );
    add_diff_if_different(
        &mut differences,
        "burst_multiplier",
        &current.burst_multiplier,
        &compare_with.burst_multiplier,
    );

    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.admin",
        current.tier_multipliers.admin,
        compare_with.tier_multipliers.admin,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.user",
        current.tier_multipliers.user,
        compare_with.tier_multipliers.user,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.a2a",
        current.tier_multipliers.a2a,
        compare_with.tier_multipliers.a2a,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.mcp",
        current.tier_multipliers.mcp,
        compare_with.tier_multipliers.mcp,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.service",
        current.tier_multipliers.service,
        compare_with.tier_multipliers.service,
    );
    add_diff_if_different_f64(
        &mut differences,
        "tier_multipliers.anon",
        current.tier_multipliers.anon,
        compare_with.tier_multipliers.anon,
    );

    let output = DiffOutput {
        source,
        differences: differences.clone(),
        identical: differences.is_empty(),
    };

    render_result(&CommandResult::table(output).with_title("Rate Limits Diff"));

    if config.output_format() == OutputFormat::Table {
        if differences.is_empty() {
            CliService::success("No differences found");
        } else {
            CliService::info(&format!("{} difference(s) found", differences.len()));
        }
    }

    Ok(())
}

fn add_diff_if_different<T: std::fmt::Display + PartialEq>(
    diffs: &mut Vec<DiffEntry>,
    field: &str,
    current: &T,
    compare: &T,
) {
    if current != compare {
        diffs.push(DiffEntry {
            field: field.to_string(),
            current: current.to_string(),
            other: compare.to_string(),
        });
    }
}

fn add_diff_if_different_f64(diffs: &mut Vec<DiffEntry>, field: &str, current: f64, compare: f64) {
    if (current - compare).abs() > f64::EPSILON {
        diffs.push(DiffEntry {
            field: field.to_string(),
            current: format!("{:.1}", current),
            other: format!("{:.1}", compare),
        });
    }
}
