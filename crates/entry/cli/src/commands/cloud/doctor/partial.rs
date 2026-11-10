//! The preflight over a profile whose variables are not all set on this host.
//!
//! A profile with every value resolved runs the full preflight. Otherwise each
//! check runs when the sections it reads resolve here: the secrets source and
//! everything derived from it (required keys, signing key, database probes),
//! and the signing key path. A check that needs the whole typed profile, or a
//! section holding an unresolved value, is reported as not evaluated, naming
//! the variables it needs, so the report is never a clean pass built from
//! checks that did not run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use systemprompt_manifest::profile::{
    DEFAULT_SIGNING_KEY_PATH, ManagedProfile, ProfileError, SecretsConfig,
};

use super::checks::{check_database_reachable, check_required_secrets, check_signing_key_at};
use super::{CheckResult, DoctorReport, resolve_secrets_from, restore_checks, run};

const WHOLE_PROFILE_CHECKS: &[&str] = &[
    "profile",
    "proxy-topology",
    "hook-url",
    "instance-id",
    "trusted-proxies",
    "storage-backup",
    "providers",
    "extension-config",
    "config-planes",
    "services-source",
];

const SECRETS_CHECKS: &[&str] = &["secrets", "signing-key", "database", "schema-version"];

pub async fn run_managed(
    managed: &ManagedProfile,
    profile_dir: &Path,
    distributed: bool,
) -> DoctorReport {
    if managed.unresolved().is_empty() {
        return match managed.profile() {
            Ok(profile) => run(&profile, profile_dir, distributed).await,
            Err(error) => DoctorReport {
                checks: vec![CheckResult::fail("profile", format!("{error}"))],
            },
        };
    }
    run_partial(managed, profile_dir, distributed).await
}

async fn run_partial(
    managed: &ManagedProfile,
    profile_dir: &Path,
    distributed: bool,
) -> DoctorReport {
    let all_unresolved = variables(managed.unresolved().iter().map(|e| e.variable.as_str()));
    let mut checks: Vec<CheckResult> = WHOLE_PROFILE_CHECKS
        .iter()
        .map(|name| CheckResult::not_evaluated(name, &all_unresolved))
        .collect();
    if distributed {
        checks.extend(
            [
                "identity-fingerprints",
                "session-url",
                "write-primary",
                "replica-lag",
                "readyz",
            ]
            .iter()
            .map(|name| CheckResult::not_evaluated(name, &all_unresolved)),
        );
    }

    match managed.section::<SecretsConfig>("secrets") {
        Ok(secrets_config) => {
            let secrets =
                resolve_secrets_from(secrets_config.as_ref(), profile_dir, &mut checks).await;
            checks.push(check_required_secrets(&secrets));
            checks.push(signing_key_check(managed, profile_dir, &secrets));
            checks.push(check_database_reachable(&secrets).await);
            checks.push(restore_checks::check_schema_not_ahead(&secrets).await);
        },
        Err(ProfileError::Interpolation { .. }) => {
            let needed = variables(
                managed
                    .unresolved()
                    .iter()
                    .filter(|e| e.key_path.starts_with("secrets."))
                    .map(|e| e.variable.as_str()),
            );
            checks.extend(
                SECRETS_CHECKS
                    .iter()
                    .map(|name| CheckResult::not_evaluated(name, &needed)),
            );
        },
        Err(error) => checks.push(CheckResult::fail("secrets", format!("{error}"))),
    }

    DoctorReport { checks }
}

fn signing_key_check(
    managed: &ManagedProfile,
    profile_dir: &Path,
    secrets: &HashMap<String, String>,
) -> CheckResult {
    match managed.section::<PathBuf>("security.signing_key_path") {
        Ok(configured) => {
            let configured = configured.unwrap_or_else(|| PathBuf::from(DEFAULT_SIGNING_KEY_PATH));
            let path = if configured.is_absolute() {
                configured
            } else {
                profile_dir.join(configured)
            };
            check_signing_key_at(&path, secrets)
        },
        Err(ProfileError::Interpolation { source, .. }) => CheckResult::not_evaluated(
            "signing-key",
            &variables(std::iter::once(source.variable.as_str())),
        ),
        Err(error) => CheckResult::fail("signing-key", format!("{error}")),
    }
}

fn variables<'a>(names: impl Iterator<Item = &'a str>) -> String {
    let mut names: Vec<&str> = names.collect();
    names.sort_unstable();
    names.dedup();
    names
        .iter()
        .map(|name| format!("${{{name}}}"))
        .collect::<Vec<_>>()
        .join(", ")
}
