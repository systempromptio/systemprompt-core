//! `cloud doctor`: pre-deploy preflight for runtime prerequisites.
//!
//! Validates the things that otherwise only surface as a post-deploy 500 — a
//! valid profile (incl. `governance.authz`), a provisionable signing key,
//! `secrets.json` with the required keys and provider credentials — and probes
//! database/hook reachability. The preflight runs automatically before
//! `cloud deploy` builds an image, and is exposed standalone (`cloud doctor`)
//! so an operator can check a profile without deploying.

mod checks;

pub(in crate::commands::cloud) use checks::resolve_signing_key_path;
pub use checks::{
    check_profile_valid, check_provider_secrets, check_required_secrets, check_signing_key,
};

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use systemprompt_cloud::ProfilePath;
use systemprompt_logging::CliService;
use systemprompt_models::Profile;

use super::deploy::resolve_profile;
use crate::cli_settings::CliConfig;
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

pub(in crate::commands::cloud) async fn run(profile: &Profile, profile_dir: &Path) -> DoctorReport {
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
    checks.push(check_provider_secrets(profile, &secrets));
    checks.push(checks::check_governance_hook_url(profile));
    checks.push(checks::check_database_reachable(&secrets).await);

    DoctorReport { checks }
}

pub(in crate::commands::cloud) async fn execute(
    profile_name: Option<String>,
    config: &CliConfig,
) -> Result<()> {
    let (profile, profile_path) = resolve_profile(profile_name.as_deref(), config)?;
    let profile_dir = profile_path
        .parent()
        .ok_or_else(|| anyhow!("Invalid profile path"))?;

    let report = run(&profile, profile_dir).await;
    report.render();

    if report.has_blocking() {
        bail!("Deploy preflight failed — fix the items above before deploying.");
    }
    CliService::success("Deploy preflight passed");
    Ok(())
}
