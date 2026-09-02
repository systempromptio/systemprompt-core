//! Multi-replica checks: everything a second node must agree with the first on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::hash::BuildHasher;
use std::time::Duration;

use sha2::{Digest, Sha256};
use systemprompt_database::{PostgresProvider, replica_status};
use systemprompt_models::Profile;

use super::CheckResult;

const IDENTITY_SECRETS: [&str; 3] = [
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "signing_key_pem",
];
const REPLICA_LAG_WARN_SECS: f64 = 5.0;
const READYZ_TIMEOUT: Duration = Duration::from_secs(5);

fn fingerprint(value: &str) -> String {
    hex::encode(&Sha256::digest(value.as_bytes())[..8])
}

pub fn check_identity_fingerprints<S: BuildHasher>(
    secrets: &HashMap<String, String, S>,
) -> CheckResult {
    let mut missing = Vec::new();
    let mut prints = Vec::new();
    for name in IDENTITY_SECRETS {
        match secrets.get(name).filter(|v| !v.is_empty()) {
            Some(value) => prints.push(format!("{name}={}", fingerprint(value))),
            None => missing.push(name),
        }
    }
    if missing.is_empty() {
        CheckResult::pass(
            "identity-fingerprints",
            format!(
                "compare across nodes, every value must match: {}",
                prints.join(" ")
            ),
        )
    } else {
        CheckResult::fail(
            "identity-fingerprints",
            format!(
                "missing {}; every replica must share one identity — run `systemprompt admin \
                 identity generate --json` once and distribute the values",
                missing.join(", ")
            ),
        )
    }
}

pub fn check_instance_id(profile: &Profile) -> CheckResult {
    profile
        .server
        .instance_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map_or_else(
            || {
                CheckResult::warn(
                    "instance-id",
                    "server.instance_id is not set; the replica falls back to HOSTNAME, which \
                     must be stable across restarts on this platform",
                )
            },
            |id| CheckResult::pass("instance-id", format!("server.instance_id = {id}")),
        )
}

pub fn check_trusted_proxies(profile: &Profile) -> CheckResult {
    if profile.server.trusted_proxies.is_empty() {
        CheckResult::fail(
            "trusted-proxies",
            "server.trusted_proxies is empty; every caller behind the balancer would share one \
             rate-limit bucket and one ban target",
        )
    } else {
        CheckResult::pass(
            "trusted-proxies",
            format!(
                "{} range(s) configured",
                profile.server.trusted_proxies.len()
            ),
        )
    }
}

pub async fn check_write_primary<S: BuildHasher + Sync>(
    secrets: &HashMap<String, String, S>,
) -> CheckResult {
    let Some(url) = secrets.get("database_write_url").filter(|v| !v.is_empty()) else {
        return CheckResult::fail(
            "write-primary",
            "database_write_url is not set; with regional replicas the write pool must be \
             pinned to the primary explicitly",
        );
    };
    match PostgresProvider::new(url).await {
        Err(err) => CheckResult::warn(
            "write-primary",
            format!("could not connect to database_write_url ({err}); fine if run off-host"),
        ),
        Ok(provider) => match replica_status(&provider).await {
            Ok(status) if status.in_recovery => CheckResult::fail(
                "write-primary",
                "database_write_url points at a standby; writes, migrations and LISTEN/NOTIFY \
                 need the primary",
            ),
            Ok(_) => CheckResult::pass("write-primary", "database_write_url is a primary"),
            Err(err) => CheckResult::warn("write-primary", format!("probe failed: {err}")),
        },
    }
}

pub async fn check_replica_lag<S: BuildHasher + Sync>(
    secrets: &HashMap<String, String, S>,
) -> CheckResult {
    let read = secrets.get("database_url").filter(|v| !v.is_empty());
    let write = secrets.get("database_write_url").filter(|v| !v.is_empty());
    let Some(read_url) = read else {
        return CheckResult::fail("replica-lag", "database_url is not set");
    };
    if write.is_none_or(|w| w == read_url) {
        return CheckResult::pass(
            "replica-lag",
            "database_url is the primary; no replica reads configured",
        );
    }
    match PostgresProvider::new(read_url).await {
        Err(err) => CheckResult::warn(
            "replica-lag",
            format!("could not connect to database_url ({err}); fine if run off-host"),
        ),
        Ok(provider) => match replica_status(&provider).await {
            Ok(status) if !status.in_recovery => CheckResult::warn(
                "replica-lag",
                "database_url differs from database_write_url but is not a standby",
            ),
            Ok(status) => match status.replay_lag_secs {
                Some(lag) if lag > REPLICA_LAG_WARN_SECS => CheckResult::warn(
                    "replica-lag",
                    format!(
                        "standby replay lag is {lag:.1}s (warn above {REPLICA_LAG_WARN_SECS}s)"
                    ),
                ),
                Some(lag) => {
                    CheckResult::pass("replica-lag", format!("standby replay lag {lag:.1}s"))
                },
                None => CheckResult::pass("replica-lag", "standby has replayed nothing yet"),
            },
            Err(err) => CheckResult::warn("replica-lag", format!("probe failed: {err}")),
        },
    }
}

pub async fn check_readyz(profile: &Profile) -> CheckResult {
    let url = format!(
        "{}/readyz",
        profile.server.api_internal_url.trim_end_matches('/')
    );
    let client = match reqwest::Client::builder().timeout(READYZ_TIMEOUT).build() {
        Ok(client) => client,
        Err(err) => return CheckResult::warn("readyz", format!("http client: {err}")),
    };
    match client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            CheckResult::pass("readyz", format!("{url} answered {}", response.status()))
        },
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            CheckResult::warn("readyz", format!("{url} answered {status}: {body}"))
        },
        Err(err) => CheckResult::warn(
            "readyz",
            format!("{url} unreachable ({err}); fine if run off-host"),
        ),
    }
}

pub async fn run<S: BuildHasher + Sync>(
    profile: &Profile,
    secrets: &HashMap<String, String, S>,
) -> Vec<CheckResult> {
    vec![
        check_identity_fingerprints(secrets),
        check_instance_id(profile),
        check_trusted_proxies(profile),
        check_write_primary(secrets).await,
        check_replica_lag(secrets).await,
        check_readyz(profile).await,
    ]
}
