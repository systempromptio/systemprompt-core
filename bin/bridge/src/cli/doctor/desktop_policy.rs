//! Whether the installed Claude Desktop supports the signed fleet settings.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::Check;
use crate::integration::host_app::ProbeEnv;

// Why: whether the installed Desktop settings match is the probe's verdict,
// reported with every other host under "host profile secret"; this check adds
// only what the probe cannot know — whether the installed Desktop understands
// the fleet settings at all.
pub(super) fn check_settings(env: &ProbeEnv) -> Vec<Check> {
    let Some(host) = crate::integration::registry::host_apps()
        .iter()
        .copied()
        .find(|host| host.id() == systemprompt_models::bridge::host::HostKind::ClaudeDesktop)
    else {
        return Vec::new();
    };
    if host.probe(env).profile_keys.is_empty() {
        return Vec::new();
    }
    match crate::install::mdm::desktop_policy::verified_operator_policy() {
        Ok(policy) => version_checks(&policy),
        Err(error) => vec![Check::fail("Desktop fleet policy", error.to_string())],
    }
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
