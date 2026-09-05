//! `cloud doctor`: pre-deploy preflight for runtime prerequisites.
//!
//! Validates the things that otherwise only surface as a post-deploy 500 — a
//! valid profile (incl. `governance.authz`), a provisionable signing key,
//! `secrets.json` with the required keys and provider credentials, a
//! `trusted_proxies` set that covers the Fly peer range — and probes
//! database/hook reachability. The preflight runs automatically before
//! `cloud deploy` builds an image, and is exposed standalone (`cloud doctor`)
//! so an operator can check a profile without deploying.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod checks;
pub mod distributed;

pub(in crate::commands::cloud) use checks::resolve_signing_key_path;
pub use checks::{
    check_extension_configs, check_profile_valid, check_provider_secrets, check_proxy_topology,
    check_required_secrets, check_signing_key,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use systemprompt_cloud::{ProfilePath, ProjectContext};
use systemprompt_loader::ConfigLoader;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::deploy::resolve_profile;
use crate::cli_settings::CliConfig;
use crate::interactive::Prompter;
use systemprompt_cloud::secrets_env::load_secrets_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

pub(in crate::commands::cloud) struct DoctorReport {
    checks: Vec<CheckResult>,
}

impl DoctorReport {
    pub(in crate::commands::cloud) fn has_blocking(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }

    pub(in crate::commands::cloud) fn render(&self) {
        CliService::section("Deploy preflight");
        for check in &self.checks {
            let line = format!("{}: {}", check.name, check.detail);
            match check.status {
                CheckStatus::Pass => CliService::success(&line),
                CheckStatus::Warn => CliService::warning(&line),
                CheckStatus::Fail => CliService::error(&line),
            }
        }
    }
}

// Why: a cloud profile's `paths.config()` is the container's `/app/services`
// tree, which does not exist on the machine running `cloud doctor`. That made
// the provider-credential check — the one that would have named a missing
// Vertex credential before a deploy — degrade to a warning and report green,
// so two undeployable providers shipped. The catalog ships from the repo's
// services tree, so when the profile's own path is absent, check the local one
// rather than giving up. Falling back to the declared path keeps the warning
// (naming the path the profile asked for) when neither exists.
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

pub(in crate::commands::cloud) async fn run(
    profile: &Profile,
    profile_dir: &Path,
    distributed: bool,
) -> DoctorReport {
    let mut checks = vec![check_profile_valid(profile)];

    let secrets_path = ProfilePath::Secrets.resolve(profile_dir);
    let secrets = load_secrets_json(&secrets_path).unwrap_or_else(|_| {
        checks.push(CheckResult::fail(
            "secrets-file",
            format!(
                "secrets.json not found or unreadable at {}",
                secrets_path.display()
            ),
        ));
        HashMap::new()
    });

    checks.push(check_required_secrets(&secrets));
    checks.push(check_signing_key(profile, profile_dir, &secrets));
    let services_root = resolve_services_config(profile);
    match ConfigLoader::load_from_path(&services_root) {
        Ok(services) => checks.push(check_provider_secrets(&services.providers, &secrets)),
        Err(err) => checks.push(CheckResult::warn(
            "providers",
            format!(
                "services config at {} could not be loaded, so provider credentials were not \
                 checked: {err}",
                services_root.display()
            ),
        )),
    }
    checks.push(check_extension_configs(profile));
    checks.push(check_proxy_topology(profile));
    checks.push(checks::check_governance_hook_url(profile));
    checks.push(checks::check_database_reachable(&secrets).await);
    if distributed {
        checks.extend(distributed::run(profile, &secrets).await);
    }

    DoctorReport { checks }
}

pub(in crate::commands::cloud) async fn execute(
    profile_name: Option<String>,
    distributed: bool,
    prompter: &dyn Prompter,
    config: &CliConfig,
) -> Result<()> {
    let (profile, profile_path) = resolve_profile(prompter, profile_name.as_deref(), config)?;
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;

    let report = run(&profile, profile_dir, distributed).await;
    report.render();

    if report.has_blocking() {
        bail!("Deploy preflight failed — fix the items above before deploying.");
    }
    CliService::success("Deploy preflight passed");
    Ok(())
}
