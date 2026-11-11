//! `cloud doctor`: pre-deploy preflight for runtime prerequisites.
//!
//! Validates the things that otherwise only surface as a post-deploy 500 — a
//! valid profile (incl. `governance.authz`), a reachable secrets source with
//! the required keys and provider credentials, a
//! `trusted_proxies` set that covers the Fly peer range — and probes
//! database/hook reachability, and reports the profile's `unapplied_planes`
//! mode with whether each runtime config plane has been applied. It fails a
//! database whose schema is newer than the binary and warns when a split-role
//! deployment keeps files on node-local storage. The preflight
//! runs automatically before `cloud deploy` builds an image, and is exposed
//! standalone (`cloud doctor`) so an operator can check a profile without
//! deploying. The profile-only checks (`profile_checks`) never fail a profile
//! that `Profile::validate` accepts, so the two cannot disagree about what is
//! deployable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod checks;
pub mod distributed;
pub mod git_credential_checks;
mod partial;
mod plane_checks;
pub mod restore_checks;
pub mod vault_checks;

pub use checks::{
    check_extension_configs, check_profile_valid, check_provider_secrets, check_proxy_topology,
    check_required_secrets, check_signing_key,
};
pub use partial::run_managed;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_identifiers::ProfileName;
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;
use systemprompt_manifest::Profile;
use systemprompt_manifest::profile::SecretsSource;

use super::deploy::resolve_profile;
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use systemprompt_cloud::secrets_env::load_secrets_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    NotEvaluated,
}

#[derive(Debug)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

/// The outcome of the deploy preflight: one result per check.
#[derive(Debug)]
pub struct DoctorReport {
    checks: Vec<CheckResult>,
}

impl DoctorReport {
    #[must_use]
    pub fn checks(&self) -> &[CheckResult] {
        &self.checks
    }

    #[must_use]
    pub fn has_blocking(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }

    #[must_use]
    pub fn not_evaluated(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::NotEvaluated)
            .count()
    }

    pub(in crate::commands::cloud) fn render_summary(&self, passed: &str) {
        match self.not_evaluated() {
            0 => CliService::success(passed),
            skipped => CliService::warning(&format!(
                "Preflight incomplete: {skipped} of {} checks were not evaluated because they \
                 need variables that are not set on this host; they run when the profile boots \
                 on its target",
                self.checks.len()
            )),
        }
    }

    pub(in crate::commands::cloud) fn render(&self) {
        CliService::section("Deploy preflight");
        for check in &self.checks {
            let line = format!("{}: {}", check.name, check.detail);
            match check.status {
                CheckStatus::Pass => CliService::success(&line),
                CheckStatus::Warn => CliService::warning(&line),
                CheckStatus::Fail => CliService::error(&line),
                CheckStatus::NotEvaluated => {
                    CliService::warning(&format!("{line} (not evaluated)"));
                },
            }
        }
    }
}

fn resolve_services_config(profile: &Profile) -> PathBuf {
    let declared = PathBuf::from(profile.paths.config());
    if declared.exists() {
        return declared;
    }
    let local = ProjectContext::discover()
        .root()
        .join("services")
        .join("config")
        .join("config.yaml");
    if local.exists() {
        return local;
    }
    declared
}

async fn resolve_secrets(
    profile: &Profile,
    profile_dir: &Path,
    checks: &mut Vec<CheckResult>,
) -> HashMap<String, String> {
    resolve_secrets_from(profile.secrets.as_ref(), profile_dir, checks).await
}

async fn resolve_secrets_from(
    secrets: Option<&systemprompt_manifest::profile::SecretsConfig>,
    profile_dir: &Path,
    checks: &mut Vec<CheckResult>,
) -> HashMap<String, String> {
    let vault = secrets
        .filter(|config| matches!(config.source, SecretsSource::Vault))
        .and_then(|config| config.vault.as_ref());

    let Some(vault) = vault else {
        let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
        return load_secrets_json(&secrets_path).unwrap_or_else(|_e| {
            checks.push(CheckResult::fail(
                "secrets-file",
                format!(
                    "secrets.json not found or unreadable at {}",
                    secrets_path.display()
                ),
            ));
            HashMap::new()
        });
    };

    let address = vault_checks::check_vault_address(vault);
    let blocked = address.status == CheckStatus::Fail;
    checks.push(address);
    if blocked {
        return HashMap::new();
    }

    let (document, values) = vault_checks::check_vault_document(vault).await;
    checks.push(document);
    values
}

pub fn profile_checks(profile: &Profile) -> Vec<CheckResult> {
    vec![
        check_profile_valid(profile),
        check_proxy_topology(profile),
        checks::check_governance_hook_url(profile),
        distributed::check_instance_id(profile),
        distributed::check_trusted_proxies(profile),
        restore_checks::check_storage_backup(profile),
    ]
}

fn providers_check(profile: &Profile, secrets: &HashMap<String, String>) -> CheckResult {
    let services_root = resolve_services_config(profile);
    match ConfigLoader::load_from_path(&services_root, Some(profile)) {
        Ok(services) => check_provider_secrets(&services.credentials, secrets),
        Err(err) => CheckResult::warn(
            "providers",
            format!(
                "services config at {} could not be loaded, so provider credentials were not \
                 checked: {err}",
                services_root.display()
            ),
        ),
    }
}

pub(in crate::commands::cloud) async fn run(
    profile: &Profile,
    profile_dir: &Path,
    distributed: bool,
) -> DoctorReport {
    let mut checks = profile_checks(profile);
    let secrets = resolve_secrets(profile, profile_dir, &mut checks).await;

    checks.push(check_required_secrets(&secrets));
    checks.push(check_signing_key(profile, profile_dir, &secrets));
    checks.push(providers_check(profile, &secrets));
    checks.push(check_extension_configs(profile));
    checks.push(checks::check_database_reachable(&secrets).await);
    checks.push(restore_checks::check_schema_not_ahead(&secrets).await);
    checks.push(plane_checks::check_config_planes(profile, &secrets).await);
    checks.push(plane_checks::check_services_source(profile));
    checks.push(git_credential_checks::check_git_credentials(&secrets).await);
    if distributed {
        checks.extend(distributed::run(profile, &secrets).await);
    }

    DoctorReport { checks }
}

pub(in crate::commands::cloud) async fn execute(
    profile_name: Option<ProfileName>,
    distributed: bool,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    let profile = resolve_profile(prompter, profile_name.as_ref(), config)?;
    let report = run_managed(&profile.managed, profile.profile_dir(), distributed).await;
    report.render();

    if report.has_blocking() {
        bail!("Deploy preflight failed — fix the items above before deploying.");
    }
    report.render_summary("Deploy preflight passed");
    Ok(())
}
