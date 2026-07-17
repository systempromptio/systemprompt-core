//! Individual pre-deploy checks.
//!
//! Each function returns a [`CheckResult`]. Configuration prerequisites that
//! would otherwise surface only as a post-deploy 500 (signing key, governance,
//! secrets, provider credentials) are `Fail`; reachability probes whose outcome
//! depends on where the operator is running the CLI (database TCP, hook host)
//! are `Warn` so they inform without blocking a legitimate deploy.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::path::{Path, PathBuf};
use std::time::Duration;

use systemprompt_models::Profile;

use super::{CheckResult, CheckStatus};

pub fn check_profile_valid(profile: &Profile) -> CheckResult {
    match profile.validate() {
        Ok(()) => CheckResult::pass("profile", "schema and required fields valid"),
        Err(err) => CheckResult::fail("profile", err.to_string()),
    }
}

pub fn check_extension_configs(profile: &Profile) -> CheckResult {
    let services_path = Path::new(&profile.paths.services);
    match systemprompt_runtime::validate_extension_configs(services_path) {
        Err(err) => CheckResult::fail(
            "extension-config",
            format!("could not discover extensions: {err}"),
        ),
        Ok(outcomes) => {
            let failures: Vec<String> = outcomes
                .iter()
                .filter_map(|o| {
                    o.error
                        .as_ref()
                        .map(|msg| format!("[ext:{}] {msg}", o.extension_id))
                })
                .collect();
            if failures.is_empty() {
                CheckResult::pass("extension-config", "all extension configs valid")
            } else {
                CheckResult::fail("extension-config", failures.join("\n"))
            }
        },
    }
}

pub(in crate::commands::cloud) fn resolve_signing_key_path(
    profile: &Profile,
    profile_dir: &Path,
) -> PathBuf {
    let configured = &profile.security.signing_key_path;
    if configured.is_absolute() {
        configured.clone()
    } else {
        profile_dir.join(configured)
    }
}

pub fn check_signing_key<S: BuildHasher>(
    profile: &Profile,
    profile_dir: &Path,
    secrets: &HashMap<String, String, S>,
) -> CheckResult {
    if secrets.contains_key("signing_key_pem") {
        return CheckResult::pass("signing-key", "provided via secrets.json (signing_key_pem)");
    }

    let path = resolve_signing_key_path(profile, profile_dir);
    if path.exists() {
        CheckResult::pass("signing-key", path.display().to_string())
    } else {
        CheckResult::fail(
            "signing-key",
            format!(
                "no signing key at {} and no signing_key_pem in secrets.json — the deploy cannot \
                 provision a JWT signing key, so every request would 500. Generate one with \
                 `systemprompt admin keys generate --output {}`.",
                path.display(),
                path.display()
            ),
        )
    }
}

pub fn check_required_secrets<S: BuildHasher>(secrets: &HashMap<String, String, S>) -> CheckResult {
    let mut missing: Vec<&str> = Vec::new();

    if !secrets.contains_key("oauth_at_rest_pepper") {
        missing.push("oauth_at_rest_pepper");
    }
    let has_db =
        secrets.contains_key("database_url") || secrets.contains_key("internal_database_url");
    if !has_db {
        missing.push("database_url (or internal_database_url)");
    }

    if missing.is_empty() {
        CheckResult::pass("secrets", "required keys present")
    } else {
        CheckResult::fail(
            "secrets",
            format!(
                "secrets.json is missing required keys: {}",
                missing.join(", ")
            ),
        )
    }
}

pub fn check_provider_secrets<S: BuildHasher>(
    profile: &Profile,
    secrets: &HashMap<String, String, S>,
) -> CheckResult {
    let missing: Vec<String> = profile
        .providers
        .providers
        .iter()
        .filter(|provider| !secret_present(secrets, provider.api_key_secret.as_str()))
        .map(|provider| {
            format!(
                "{} (needs `{}`)",
                provider.name.as_str(),
                provider.api_key_secret.as_str()
            )
        })
        .collect();

    if missing.is_empty() {
        CheckResult::pass("providers", "all provider credentials present")
    } else {
        CheckResult::fail(
            "providers",
            format!(
                "secrets.json is missing credentials for: {}",
                missing.join(", ")
            ),
        )
    }
}

fn secret_present<S: BuildHasher>(secrets: &HashMap<String, String, S>, name: &str) -> bool {
    secrets.contains_key(name)
        || secrets.contains_key(&name.to_uppercase())
        || secrets.contains_key(&name.to_lowercase())
}

pub(super) async fn check_database_reachable(secrets: &HashMap<String, String>) -> CheckResult {
    let Some(url) = secrets
        .get("external_database_url")
        .or_else(|| secrets.get("database_url"))
        .or_else(|| secrets.get("internal_database_url"))
    else {
        return CheckResult::warn("database", "no database URL to probe");
    };

    let Some((host, port)) = host_port(url) else {
        return CheckResult::warn("database", "could not parse host:port from database URL");
    };

    match tokio::time::timeout(
        Duration::from_secs(5),
        tokio::net::TcpStream::connect((host.as_str(), port)),
    )
    .await
    {
        Ok(Ok(_)) => CheckResult::pass("database", format!("reachable at {host}:{port}")),
        Ok(Err(err)) => CheckResult::warn(
            "database",
            format!("{host}:{port} unreachable from here ({err}) — fine if DB is Fly-internal"),
        ),
        Err(_) => CheckResult::warn(
            "database",
            format!("{host}:{port} did not answer within 5s — fine if DB is Fly-internal"),
        ),
    }
}

pub(super) fn check_governance_hook_url(profile: &Profile) -> CheckResult {
    let Some(authz) = profile.governance.as_ref().and_then(|g| g.authz.as_ref()) else {
        return CheckResult::warn("hook-url", "no governance.authz block");
    };
    let Some(url) = authz.hook.url.as_deref().filter(|u| !u.is_empty()) else {
        return CheckResult::pass("hook-url", "no webhook URL to check for this mode");
    };

    let hook_host = host_port(url).map(|(h, _)| h);
    let external_host = host_port(&profile.server.api_external_url).map(|(h, _)| h);

    match (hook_host, external_host) {
        (Some(hook), Some(external)) if hook == external => {
            CheckResult::pass("hook-url", format!("targets {external}"))
        },
        (Some(hook), Some(external)) if is_loopback(&hook) => CheckResult::warn(
            "hook-url",
            format!(
                "points at {hook} but api_external_url is {external} — a loopback hook only works \
                 if the gateway and webhook share the machine"
            ),
        ),
        (Some(hook), Some(external)) => CheckResult::warn(
            "hook-url",
            format!("targets {hook}, but api_external_url is {external} — verify this is intended"),
        ),
        _ => CheckResult::warn("hook-url", "could not parse hook or api_external_url host"),
    }
}

fn host_port(raw: &str) -> Option<(String, u16)> {
    let parsed = url::Url::parse(raw).ok()?;
    let host = parsed.host_str()?.to_owned();
    let port = parsed.port_or_known_default()?;
    Some((host, port))
}

fn is_loopback(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

impl CheckResult {
    pub(super) fn pass(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Pass,
            detail: detail.into(),
        }
    }

    pub(super) fn warn(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Warn,
            detail: detail.into(),
        }
    }

    pub(super) fn fail(name: &'static str, detail: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            detail: detail.into(),
        }
    }
}
