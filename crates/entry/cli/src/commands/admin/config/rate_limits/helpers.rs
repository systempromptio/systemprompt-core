use anyhow::{bail, Context, Result};
use std::fs;
use systemprompt_models::profile::{RateLimitsConfig, TierMultipliers};
use systemprompt_models::Profile;

use super::super::types::ResetChange;

pub fn apply_multiplier(base: u64, multiplier: f64) -> u64 {
    (base as f64 * multiplier).round() as u64
}

pub fn get_tier_multiplier(tiers: &TierMultipliers, tier: &str) -> Result<f64> {
    match tier {
        "admin" => Ok(tiers.admin),
        "user" => Ok(tiers.user),
        "a2a" => Ok(tiers.a2a),
        "mcp" => Ok(tiers.mcp),
        "service" => Ok(tiers.service),
        "anon" => Ok(tiers.anon),
        _ => bail!(
            "Unknown tier: {}. Valid tiers: admin, user, a2a, mcp, service, anon",
            tier
        ),
    }
}

pub fn set_tier_multiplier(tiers: &mut TierMultipliers, tier: &str, value: f64) -> Result<()> {
    match tier {
        "admin" => tiers.admin = value,
        "user" => tiers.user = value,
        "a2a" => tiers.a2a = value,
        "mcp" => tiers.mcp = value,
        "service" => tiers.service = value,
        "anon" => tiers.anon = value,
        _ => bail!(
            "Unknown tier: {}. Valid tiers: admin, user, a2a, mcp, service, anon",
            tier
        ),
    }
    Ok(())
}

pub fn get_endpoint_rate(limits: &RateLimitsConfig, endpoint: &str) -> Result<u64> {
    match endpoint {
        "oauth_public" => Ok(limits.oauth_public_per_second),
        "oauth_auth" => Ok(limits.oauth_auth_per_second),
        "contexts" => Ok(limits.contexts_per_second),
        "tasks" => Ok(limits.tasks_per_second),
        "artifacts" => Ok(limits.artifacts_per_second),
        "agent_registry" => Ok(limits.agent_registry_per_second),
        "agents" => Ok(limits.agents_per_second),
        "mcp_registry" => Ok(limits.mcp_registry_per_second),
        "mcp" => Ok(limits.mcp_per_second),
        "stream" => Ok(limits.stream_per_second),
        "content" => Ok(limits.content_per_second),
        _ => bail!(
            "Unknown endpoint: {}. Valid endpoints: oauth_public, oauth_auth, contexts, tasks, \
             artifacts, agent_registry, agents, mcp_registry, mcp, stream, content",
            endpoint
        ),
    }
}

pub fn set_endpoint_rate(limits: &mut RateLimitsConfig, endpoint: &str, value: u64) -> Result<()> {
    match endpoint {
        "oauth_public" => limits.oauth_public_per_second = value,
        "oauth_auth" => limits.oauth_auth_per_second = value,
        "contexts" => limits.contexts_per_second = value,
        "tasks" => limits.tasks_per_second = value,
        "artifacts" => limits.artifacts_per_second = value,
        "agent_registry" => limits.agent_registry_per_second = value,
        "agents" => limits.agents_per_second = value,
        "mcp_registry" => limits.mcp_registry_per_second = value,
        "mcp" => limits.mcp_per_second = value,
        "stream" => limits.stream_per_second = value,
        "content" => limits.content_per_second = value,
        _ => bail!(
            "Unknown endpoint: {}. Valid endpoints: oauth_public, oauth_auth, contexts, tasks, \
             artifacts, agent_registry, agents, mcp_registry, mcp, stream, content",
            endpoint
        ),
    }
    Ok(())
}

pub fn load_profile_for_edit(path: &str) -> Result<Profile> {
    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read profile: {}", path))?;
    let profile: Profile = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse profile: {}", path))?;
    Ok(profile)
}

pub fn save_profile(profile: &Profile, path: &str) -> Result<()> {
    let content = serde_yaml::to_string(profile).context("Failed to serialize profile")?;
    fs::write(path, content).with_context(|| format!("Failed to write profile: {}", path))?;
    Ok(())
}

pub fn collect_endpoint_changes(
    current: &RateLimitsConfig,
    defaults: &RateLimitsConfig,
    changes: &mut Vec<ResetChange>,
) {
    let endpoints = [
        (
            "oauth_public_per_second",
            current.oauth_public_per_second,
            defaults.oauth_public_per_second,
        ),
        (
            "oauth_auth_per_second",
            current.oauth_auth_per_second,
            defaults.oauth_auth_per_second,
        ),
        (
            "contexts_per_second",
            current.contexts_per_second,
            defaults.contexts_per_second,
        ),
        (
            "tasks_per_second",
            current.tasks_per_second,
            defaults.tasks_per_second,
        ),
        (
            "artifacts_per_second",
            current.artifacts_per_second,
            defaults.artifacts_per_second,
        ),
        (
            "agent_registry_per_second",
            current.agent_registry_per_second,
            defaults.agent_registry_per_second,
        ),
        (
            "agents_per_second",
            current.agents_per_second,
            defaults.agents_per_second,
        ),
        (
            "mcp_registry_per_second",
            current.mcp_registry_per_second,
            defaults.mcp_registry_per_second,
        ),
        (
            "mcp_per_second",
            current.mcp_per_second,
            defaults.mcp_per_second,
        ),
        (
            "stream_per_second",
            current.stream_per_second,
            defaults.stream_per_second,
        ),
        (
            "content_per_second",
            current.content_per_second,
            defaults.content_per_second,
        ),
    ];

    for (name, current_val, default_val) in endpoints {
        if current_val != default_val {
            changes.push(ResetChange {
                field: name.to_string(),
                old_value: current_val.to_string(),
                new_value: default_val.to_string(),
            });
        }
    }
}

pub fn collect_tier_changes(
    current: &TierMultipliers,
    defaults: &TierMultipliers,
    changes: &mut Vec<ResetChange>,
) {
    let tiers = [
        ("tier_multipliers.admin", current.admin, defaults.admin),
        ("tier_multipliers.user", current.user, defaults.user),
        ("tier_multipliers.a2a", current.a2a, defaults.a2a),
        ("tier_multipliers.mcp", current.mcp, defaults.mcp),
        (
            "tier_multipliers.service",
            current.service,
            defaults.service,
        ),
        ("tier_multipliers.anon", current.anon, defaults.anon),
    ];

    for (name, current_val, default_val) in tiers {
        if (current_val - default_val).abs() > f64::EPSILON {
            changes.push(ResetChange {
                field: name.to_string(),
                old_value: format!("{:.1}", current_val),
                new_value: format!("{:.1}", default_val),
            });
        }
    }
}
