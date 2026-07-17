//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use systemprompt_config::ProfileBootstrap;
use systemprompt_logging::CliService;
use systemprompt_models::profile::{RateLimitsConfig, TierMultipliers};

use super::DiffArgs;
use crate::CliConfig;
use crate::cli_settings::OutputFormat;
use crate::shared::{CommandOutput, render_result};

use super::super::types::{DiffEntry, DiffOutput};

pub(super) fn execute_diff(args: &DiffArgs, config: &CliConfig) -> Result<()> {
    let profile = ProfileBootstrap::get()?;
    let current = &profile.rate_limits;

    let (compare_with, source) = if args.defaults {
        (RateLimitsConfig::default(), "defaults".to_owned())
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

    let differences = collect_differences(current, &compare_with);
    let count = differences.len();

    let output = DiffOutput {
        source,
        identical: differences.is_empty(),
        differences,
    };

    render_result(
        &CommandOutput::table_of(vec!["field", "current", "other"], &output.differences)
            .with_title("Rate Limits Diff"),
        config,
    );

    if config.output_format() == OutputFormat::Table {
        if count == 0 {
            CliService::success("No differences found");
        } else {
            CliService::info(&format!("{count} difference(s) found"));
        }
    }

    Ok(())
}

fn collect_differences(
    current: &RateLimitsConfig,
    compare_with: &RateLimitsConfig,
) -> Vec<DiffEntry> {
    let mut differences: Vec<DiffEntry> = Vec::new();

    add_diff_if_different(
        &mut differences,
        "disabled",
        &current.disabled,
        &compare_with.disabled,
    );
    collect_rate_differences(current, compare_with, &mut differences);
    add_diff_if_different(
        &mut differences,
        "burst_multiplier",
        &current.burst_multiplier,
        &compare_with.burst_multiplier,
    );
    collect_tier_differences(
        &current.tier_multipliers,
        &compare_with.tier_multipliers,
        &mut differences,
    );

    differences
}

fn collect_rate_differences(
    current: &RateLimitsConfig,
    compare_with: &RateLimitsConfig,
    diffs: &mut Vec<DiffEntry>,
) {
    let rates = [
        (
            "oauth_public_per_second",
            current.oauth_public_per_second,
            compare_with.oauth_public_per_second,
        ),
        (
            "oauth_auth_per_second",
            current.oauth_auth_per_second,
            compare_with.oauth_auth_per_second,
        ),
        (
            "contexts_per_second",
            current.contexts_per_second,
            compare_with.contexts_per_second,
        ),
        (
            "tasks_per_second",
            current.tasks_per_second,
            compare_with.tasks_per_second,
        ),
        (
            "artifacts_per_second",
            current.artifacts_per_second,
            compare_with.artifacts_per_second,
        ),
        (
            "agent_registry_per_second",
            current.agent_registry_per_second,
            compare_with.agent_registry_per_second,
        ),
        (
            "agents_per_second",
            current.agents_per_second,
            compare_with.agents_per_second,
        ),
        (
            "mcp_registry_per_second",
            current.mcp_registry_per_second,
            compare_with.mcp_registry_per_second,
        ),
        (
            "mcp_per_second",
            current.mcp_per_second,
            compare_with.mcp_per_second,
        ),
        (
            "stream_per_second",
            current.stream_per_second,
            compare_with.stream_per_second,
        ),
        (
            "content_per_second",
            current.content_per_second,
            compare_with.content_per_second,
        ),
    ];

    for (field, current_val, other_val) in rates {
        add_diff_if_different(diffs, field, &current_val, &other_val);
    }
}

fn collect_tier_differences(
    current: &TierMultipliers,
    compare_with: &TierMultipliers,
    diffs: &mut Vec<DiffEntry>,
) {
    let tiers = [
        ("tier_multipliers.admin", current.admin, compare_with.admin),
        ("tier_multipliers.user", current.user, compare_with.user),
        ("tier_multipliers.a2a", current.a2a, compare_with.a2a),
        ("tier_multipliers.mcp", current.mcp, compare_with.mcp),
        (
            "tier_multipliers.service",
            current.service,
            compare_with.service,
        ),
        ("tier_multipliers.anon", current.anon, compare_with.anon),
    ];

    for (field, current_val, other_val) in tiers {
        add_diff_if_different_f64(diffs, field, current_val, other_val);
    }
}

fn add_diff_if_different<T: std::fmt::Display + PartialEq>(
    diffs: &mut Vec<DiffEntry>,
    field: &str,
    current: &T,
    compare: &T,
) {
    if current != compare {
        diffs.push(DiffEntry {
            field: field.to_owned(),
            current: current.to_string(),
            other: compare.to_string(),
        });
    }
}

fn add_diff_if_different_f64(diffs: &mut Vec<DiffEntry>, field: &str, current: f64, compare: f64) {
    if (current - compare).abs() > f64::EPSILON {
        diffs.push(DiffEntry {
            field: field.to_owned(),
            current: format!("{:.1}", current),
            other: format!("{:.1}", compare),
        });
    }
}
