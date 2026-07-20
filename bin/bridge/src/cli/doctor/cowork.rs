//! Doctor checks for Cowork enablement and plugin-installation preferences.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::config::paths;

use super::Check;

// Catches the silent "plugin on disk but Cowork never enabled it" state via the
// enable keys in cowork_settings.json.
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
    let plugin_ids = synced_plugin_ids();
    if plugin_ids.is_empty() {
        return Check::warn(
            "cowork enable",
            format!(
                "no synced plugin dirs found — run `{} sync`",
                crate::brand::brand().binary_name
            ),
        );
    }
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
    let enabled_map = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|v| v.get("enabledPlugins").cloned())
        .unwrap_or_default();
    let missing: Vec<String> = plugin_ids
        .iter()
        .map(|id| enabled_plugins_key(id, ORG_PROVISIONED))
        .filter(|key| enabled_map.get(key) != Some(&serde_json::Value::Bool(true)))
        .collect();
    if missing.is_empty() {
        Check::ok(
            "cowork enable",
            format!(
                "{} plugin(s) enabled in {}",
                plugin_ids.len(),
                settings.display()
            ),
        )
    } else {
        Check::fail(
            "cowork enable",
            format!(
                "{} not set in {} — Cowork will not load those synced plugins",
                missing.join(", "),
                settings.display()
            ),
        )
    }
}

fn synced_plugin_ids() -> Vec<String> {
    let Some(location) = paths::org_plugins_effective() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&location.path) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|n| !n.starts_with('.'))
        .collect();
    ids.sort();
    ids
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
    let plugin_ids = synced_plugin_ids();
    if plugin_ids.is_empty() {
        return Check::warn(
            "plugin auto-install",
            format!(
                "no synced plugin dirs under {} — run `{} sync`",
                location.path.display(),
                crate::brand::brand().binary_name
            ),
        );
    }
    for id in &plugin_ids {
        let plugin_json = location
            .path
            .join(id)
            .join(".claude-plugin")
            .join("plugin.json");
        let Ok(text) = std::fs::read_to_string(&plugin_json) else {
            return Check::fail(
                "plugin auto-install",
                format!("{} not present", plugin_json.display()),
            );
        };
        let Ok(probe) = serde_json::from_str::<PluginManifestProbe>(&text) else {
            return Check::fail(
                "plugin auto-install",
                format!("{}: invalid JSON", plugin_json.display()),
            );
        };
        match probe.installation_preference.as_deref() {
            Some("required" | "auto_install") => {},
            Some("available") => {
                return Check::fail(
                    "plugin auto-install",
                    format!(
                        "{}: installationPreference=available — Cowork will require a manual \
                         install click, which surfaces \"Contact an organization owner\" under MDM",
                        plugin_json.display(),
                    ),
                );
            },
            Some(other) => {
                return Check::fail(
                    "plugin auto-install",
                    format!(
                        "{}: installationPreference={other} is not one of \
                         required|auto_install|available",
                        plugin_json.display(),
                    ),
                );
            },
            None => {
                return Check::fail(
                    "plugin auto-install",
                    format!(
                        "{}: installationPreference is missing — Cowork will default to \
                         \"available\" (manual install, owner-gated)",
                        plugin_json.display(),
                    ),
                );
            },
        }
    }
    Check::ok(
        "plugin auto-install",
        format!(
            "{} plugin(s) carry installationPreference=required|auto_install",
            plugin_ids.len()
        ),
    )
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
