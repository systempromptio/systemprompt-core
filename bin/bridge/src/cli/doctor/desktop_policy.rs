//! Effective Desktop fleet settings, including legacy context defaults.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Check;
use crate::integration::host_app::ProbeEnv;

pub(super) fn check_settings(env: &ProbeEnv) -> Vec<Check> {
    let Some(host) = crate::integration::registry::host_apps()
        .iter()
        .copied()
        .find(|host| host.id() == systemprompt_models::bridge::host::HostKind::ClaudeDesktop)
    else {
        return Vec::new();
    };
    let snapshot = host.probe(env);
    if snapshot.profile_keys.is_empty() {
        return Vec::new();
    }
    let policy = match crate::install::mdm::desktop_policy::verified_operator_policy() {
        Ok(policy) => policy,
        Err(error) => return vec![Check::fail("Desktop fleet policy", error.to_string())],
    };
    let expected = match crate::install::mdm::desktop_policy::resolved_settings(&policy) {
        Ok(settings) => crate::install::mdm::policy::reg_values(&settings),
        Err(error) => return vec![Check::fail("Desktop fleet policy", error.to_string())],
    };
    let different: Vec<_> = expected
        .iter()
        .filter(|(key, _, value)| snapshot.profile_keys.get(*key) != Some(value))
        .map(|(key, _, _)| *key)
        .collect();
    let mut checks = version_checks(&policy);
    if different.is_empty() {
        checks.push(Check::ok("Desktop fleet policy", "Installed settings match the signed fleet defaults. Fully restart Desktop after a profile change; test a fresh Cowork task."));
    } else {
        checks.push(Check::warn("Desktop fleet policy", format!("Missing or different settings: {}. Repair the Desktop profile, approve it on macOS, then fully restart Desktop.", different.join(", "))));
    }
    checks
}

fn version_checks(policy: &crate::install::mdm::desktop_policy::DesktopPolicy) -> Vec<Check> {
    use crate::install::mdm::desktop_policy::{
        installed_desktop_version, settings_catalog, supports_version,
    };
    let Some(version) = installed_desktop_version() else {
        return vec![Check::warn(
            "Desktop version",
            "Could not determine the installed version; verify the fleet settings in Desktop's configuration window.",
        )];
    };
    let Ok(catalog) = settings_catalog() else {
        return Vec::new();
    };
    let unsupported: Vec<_> = catalog
        .settings
        .iter()
        .filter(|setting| setting.default.is_some() || policy.settings.contains_key(&setting.key))
        .filter(|setting| {
            setting
                .min_version
                .as_ref()
                .is_some_and(|minimum| !supports_version(&version, minimum))
        })
        .map(|setting| setting.key.as_str())
        .collect();
    if unsupported.is_empty() {
        vec![Check::ok(
            "Desktop version",
            format!("{version} supports the configured fleet settings."),
        )]
    } else {
        vec![Check::warn(
            "Desktop version",
            format!(
                "{version} does not support: {}. Update Desktop before testing these settings.",
                unsupported.join(", ")
            ),
        )]
    }
}
