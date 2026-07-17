//! Doctor checks for Cowork enablement and plugin-installation preferences.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::paths;

use super::Check;

// Catches the silent "plugin on disk but Cowork never enabled it" state via the
// enable key in cowork_settings.json.
pub fn check_cowork_enable() -> Check {
    use crate::integration::cowork_plugins::{
        COWORK_SETTINGS_FILE, enabled_plugins_key, resolve_target,
    };
    const ORG_PROVISIONED: &str = "org-provisioned";
    let Some(target) = resolve_target() else {
        return Check::warn(
            "cowork enable",
            "no active Cowork session detected — open Claude Cowork at least once before sync",
        );
    };
    let settings = target.session_org_dir.join(COWORK_SETTINGS_FILE);
    let key = enabled_plugins_key(paths::SYNTHETIC_PLUGIN_NAME, ORG_PROVISIONED);
    let Ok(text) = std::fs::read_to_string(&settings) else {
        return Check::warn(
            "cowork enable",
            format!(
                "{} not yet written — run `{} sync`",
                settings.display(),
                crate::brand::brand().binary_name
            ),
        );
    };
    let enabled = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("enabledPlugins").cloned())
        .and_then(|v| v.get(&key).cloned())
        == Some(serde_json::Value::Bool(true));
    if enabled {
        Check::ok(
            "cowork enable",
            format!("{key} = true in {}", settings.display()),
        )
    } else {
        Check::fail(
            "cowork enable",
            format!(
                "{key} not set in {} — Cowork will not load the synced plugin",
                settings.display()
            ),
        )
    }
}

#[derive(serde::Deserialize)]
struct PluginManifestProbe {
    #[serde(rename = "installationPreference")]
    installation_preference: Option<String>,
}

// A synced plugin.json lacking `installationPreference` triggers Cowork's
// "Contact an organization owner" tooltip under MDM.
pub fn check_plugin_installation_preference() -> Check {
    let Some(location) = paths::org_plugins_effective() else {
        return Check::warn("plugin auto-install", "no org-plugins location resolvable");
    };
    let plugin_json = location
        .path
        .join(paths::SYNTHETIC_PLUGIN_NAME)
        .join(".claude-plugin")
        .join("plugin.json");
    let Ok(text) = std::fs::read_to_string(&plugin_json) else {
        return Check::warn(
            "plugin auto-install",
            format!(
                "{} not present — run `{} sync`",
                plugin_json.display(),
                crate::brand::brand().binary_name
            ),
        );
    };
    let Ok(probe) = serde_json::from_str::<PluginManifestProbe>(&text) else {
        return Check::fail(
            "plugin auto-install",
            format!("{}: invalid JSON", plugin_json.display()),
        );
    };
    match probe.installation_preference.as_deref() {
        Some(pref @ ("required" | "auto_install")) => Check::ok(
            "plugin auto-install",
            format!(
                "{} has installationPreference={pref}",
                plugin_json.display(),
            ),
        ),
        Some("available") => Check::fail(
            "plugin auto-install",
            format!(
                "{}: installationPreference=available — Cowork will require a manual install \
                 click, which surfaces \"Contact an organization owner\" under MDM",
                plugin_json.display(),
            ),
        ),
        Some(other) => Check::fail(
            "plugin auto-install",
            format!(
                "{}: installationPreference={other} is not one of required|auto_install|available",
                plugin_json.display(),
            ),
        ),
        None => Check::fail(
            "plugin auto-install",
            format!(
                "{}: installationPreference is missing — Cowork will default to \"available\" \
                 (manual install, owner-gated)",
                plugin_json.display(),
            ),
        ),
    }
}

// Warns when Cowork sessions exist but none matches the hard-coded
// PERSONAL_SESSION_UUID (Cowork may have bumped it).
pub fn check_personal_session_sentinel() -> Check {
    use crate::integration::cowork_plugins::PERSONAL_SESSION_UUID;

    let Some(root) = paths::cowork3p_sessions_root() else {
        return Check::warn(
            "personal-session sentinel",
            "no Cowork sessions root resolvable (Cowork not installed?)",
        );
    };
    if !root.is_dir() {
        return Check::warn(
            "personal-session sentinel",
            format!("{} not present — open Cowork at least once", root.display()),
        );
    }
    let mut total_orgs = 0usize;
    let mut matched = false;
    if let Ok(accounts) = std::fs::read_dir(&root) {
        for account in accounts.flatten() {
            if !account.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let Ok(orgs) = std::fs::read_dir(account.path()) else {
                continue;
            };
            for org in orgs.flatten() {
                if !org.file_type().is_ok_and(|t| t.is_dir()) {
                    continue;
                }
                total_orgs += 1;
                let name = org.file_name();
                if name
                    .to_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(PERSONAL_SESSION_UUID))
                {
                    matched = true;
                }
            }
        }
    }
    match (total_orgs, matched) {
        (0, _) => Check::warn(
            "personal-session sentinel",
            format!(
                "{} has no org session dirs yet — open Cowork to bootstrap",
                root.display()
            ),
        ),
        (_, true) => Check::ok(
            "personal-session sentinel",
            format!(
                "{PERSONAL_SESSION_UUID} present under {} — bridge resolver matches Cowork's \
                 hard-coded constant",
                root.display()
            ),
        ),
        (n, false) => Check::fail(
            "personal-session sentinel",
            format!(
                "{n} Cowork org dir(s) under {} but none matches PERSONAL_SESSION_UUID \
                 ({PERSONAL_SESSION_UUID}) — Cowork may have bumped the constant; update \
                 bin/bridge/src/integration/cowork_plugins/emit.rs to whatever literal Cowork now \
                 hard-codes (search app.asar for the new value)",
                root.display()
            ),
        ),
    }
}
