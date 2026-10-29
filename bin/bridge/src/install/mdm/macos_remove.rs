//! Removal of the bridge's macOS configuration profile and managed
//! preferences, verified against the profile inventory and the files on disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use super::MdmError;
use super::macos::{MANAGED_PREFS_PATH, PAYLOAD_IDENTIFIER, bridge_prefs_path};

fn user_prefs_path() -> Result<std::path::PathBuf, MdmError> {
    let user = std::env::var("USER").map_err(MdmError::ManagedPrefsUserUnset)?;
    if user.is_empty() || user.contains('/') || user == "." || user == ".." {
        return Err(MdmError::ManagedPrefsUser {
            detail: format!("{user:?}"),
        });
    }
    Ok(std::path::PathBuf::from(format!(
        "/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist"
    )))
}

pub(crate) fn remove_profile() -> Result<bool, MdmError> {
    let cli_removed = !super::claude_code_settings::remove_all()?.is_empty();
    let paths = [
        std::path::PathBuf::from(MANAGED_PREFS_PATH),
        user_prefs_path()?,
        std::path::PathBuf::from(bridge_prefs_path()),
    ];
    let mut existing = Vec::new();
    for path in &paths {
        if path.try_exists().map_err(|source| MdmError::Io {
            action: "read policy",
            path: path.clone(),
            source,
        })? {
            existing.push(path);
        }
    }
    // Why: `profiles show` needs administrator rights for the system
    // inventory, but the unprivileged listing is authoritative for "nothing
    // of ours is here". Skipping the elevation prompt when there is nothing
    // to remove is the whole difference between `uninstall` on a clean
    // machine asking for a password and not.
    if existing.is_empty() && unprivileged_inventory_is_clean()? {
        return Ok(cli_removed);
    }
    let was_installed = profile_inventory()?;
    if existing.is_empty() {
        return Ok(was_installed || cli_removed);
    }
    let quote = crate::install::elevation_script::shell_quote;
    let mut script = "set -e\n".to_owned();
    for path in &existing {
        script.push_str(&format!("rm -f {}\n", quote(&path.display().to_string())));
    }
    script.push_str("/usr/bin/killall cfprefsd\n");
    crate::install::elevate::run_privileged(
        &script,
        "Bridge needs administrator privileges to remove its managed preferences.",
    )?;
    for path in &paths {
        if path.try_exists().map_err(|source| MdmError::Io {
            action: "verify removal",
            path: path.clone(),
            source,
        })? {
            return Err(MdmError::RemovalIncomplete {
                what: "managed preferences plist",
                detail: path.display().to_string(),
            });
        }
    }
    Ok(true)
}

// Why: the privileged script records the inventory before and after the
// removal so the receipt says whether the profile was ever there, and the
// after-listing proves it is gone rather than trusting `profiles remove`.
fn profile_inventory() -> Result<bool, MdmError> {
    let stage = tempfile::tempdir().map_err(|source| MdmError::Io {
        action: "stage profile removal",
        path: std::env::temp_dir(),
        source,
    })?;
    let listing = stage.path().join("profiles.json");
    let xml = stage.path().join("profiles.plist");
    let before = stage.path().join("profiles-before.json");
    let quote = crate::install::elevation_script::shell_quote;
    let script = format!(
        "set -e\n/usr/bin/profiles show -type configuration -output stdout-xml > {xml}\n/usr/bin/plutil -convert json -o {before} {xml}\nchmod 0644 {before}\n/usr/bin/profiles remove -identifier {PAYLOAD_IDENTIFIER} || true\n/usr/bin/profiles show -type configuration -output stdout-xml > {xml}\n/usr/bin/plutil -convert json -o {listing} {xml}\nchmod 0644 {listing}\n",
        xml = quote(&xml.display().to_string()),
        before = quote(&before.display().to_string()),
        listing = quote(&listing.display().to_string())
    );
    crate::install::elevate::run_privileged(
        &script,
        "Bridge needs administrator privileges to inspect and remove its managed profile.",
    )?;
    let previous = std::fs::read(&before).map_err(|source| MdmError::Io {
        action: "read initial profiles",
        path: before.clone(),
        source,
    })?;
    // JSON: `profiles show` output converted by plutil, shape owned by macOS.
    let previous: serde_json::Value =
        serde_json::from_slice(&previous).map_err(|source| MdmError::Json {
            path: before,
            source,
        })?;
    let was_installed = contains_profile_identifier(&previous);
    let bytes = std::fs::read(&listing).map_err(|source| MdmError::Io {
        action: "read profile removal evidence",
        path: listing.clone(),
        source,
    })?;
    // JSON: `profiles show` output converted by plutil, shape owned by macOS.
    let profiles: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| MdmError::Json {
            path: listing,
            source,
        })?;
    if contains_profile_identifier(&profiles) {
        return Err(MdmError::RemovalIncomplete {
            what: "configuration profile",
            detail: format!(
                "{PAYLOAD_IDENTIFIER} is still installed; remove it through System Settings or MDM"
            ),
        });
    }
    Ok(was_installed)
}

fn unprivileged_inventory_is_clean() -> Result<bool, MdmError> {
    let output = std::process::Command::new("/usr/bin/profiles")
        .args(["list", "-type", "configuration"])
        .output()
        .map_err(|source| MdmError::Io {
            action: "list profiles",
            path: std::path::PathBuf::from("/usr/bin/profiles"),
            source,
        })?;
    if !output.status.success() {
        return Ok(false);
    }
    Ok(!String::from_utf8_lossy(&output.stdout).contains(PAYLOAD_IDENTIFIER))
}

// JSON: `profiles show -output stdout-xml` converted by plutil; the identifier
// can sit at any depth, so the walk is shape-agnostic.
fn contains_profile_identifier(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => value == PAYLOAD_IDENTIFIER,
        serde_json::Value::Array(values) => values.iter().any(contains_profile_identifier),
        serde_json::Value::Object(values) => values.values().any(contains_profile_identifier),
        _ => false,
    }
}

// Why: Claude Desktop reads both the per-user and the machine managed plist,
// and only an administrator can change either; one privileged script edits
// every file the bridge wrote and restarts cfprefsd once.
pub(crate) fn clear_bridge_preferences(ours: Option<&str>) -> Result<Vec<String>, MdmError> {
    use super::clear_plan::{ClearPlan, plan_clear};

    let quote = crate::install::elevation_script::shell_quote;
    let mut script = "set -e\n".to_owned();
    let mut lines = Vec::new();
    let mut touched = Vec::new();
    for path in [
        std::path::PathBuf::from(MANAGED_PREFS_PATH),
        user_prefs_path()?,
    ] {
        let observed = observe_plist(&path)?;
        let shown = path.display().to_string();
        match plan_clear(observed.as_ref(), ours) {
            ClearPlan::NothingWritten => {},
            ClearPlan::ManagedElsewhere => {
                lines.push(format!(
                    "{shown}: managed by an administrator; left in place"
                ));
            },
            ClearPlan::StaleBridgePolicy => {
                lines.push(format!(
                    "{shown}: a previous bridge install's policy; replace it from the app to release it"
                ));
            },
            ClearPlan::RemoveAll => {
                script.push_str(&format!("rm -f {}\n", quote(&shown)));
                lines.push(format!("{shown}: removed"));
                touched.push((path, None));
            },
            ClearPlan::RemoveKeys(keys) => {
                for key in &keys {
                    script.push_str(&format!(
                        "/usr/libexec/PlistBuddy -c {} {}\n",
                        quote(&format!("Delete :{key}")),
                        quote(&shown)
                    ));
                }
                lines.push(format!("{shown}: removed {} bridge key(s)", keys.len()));
                touched.push((path, Some(keys)));
            },
        }
    }
    let profile_installed = !unprivileged_inventory_is_clean()?;
    if profile_installed {
        script.push_str(&format!(
            "/usr/bin/profiles remove -identifier {} || true\n",
            quote(PAYLOAD_IDENTIFIER)
        ));
    }
    if touched.is_empty() && !profile_installed {
        return Ok(lines);
    }
    script.push_str("/usr/bin/killall cfprefsd || true\n");
    crate::install::elevate::run_privileged(
        &script,
        "Bridge needs administrator privileges to remove its Claude Desktop managed preferences.",
    )?;
    // Why: a profile an MDM pushed under the same identifier cannot be removed
    // locally; it stays and is reported as managed, not as a failed clear.
    if profile_installed {
        if unprivileged_inventory_is_clean()? {
            lines.push(format!(
                "configuration profile {PAYLOAD_IDENTIFIER}: removed"
            ));
        } else {
            lines.push(format!(
                "configuration profile {PAYLOAD_IDENTIFIER}: installed by device management; left in place"
            ));
        }
    }
    for (path, keys) in touched {
        let after = observe_plist(&path)?;
        let left = match (&keys, &after) {
            (None, Some(_)) => true,
            (Some(keys), Some(after)) => keys.iter().any(|k| after.keys.contains(*k)),
            (_, None) => false,
        };
        if left {
            return Err(MdmError::RemovalIncomplete {
                what: "Claude Desktop managed preferences",
                detail: path.display().to_string(),
            });
        }
    }
    Ok(lines)
}

fn observe_plist(
    path: &std::path::Path,
) -> Result<Option<super::clear_plan::ObservedPolicy>, MdmError> {
    if !path.try_exists().map_err(|source| MdmError::Io {
        action: "read policy",
        path: path.to_path_buf(),
        source,
    })? {
        return Ok(None);
    }
    let output = std::process::Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(path)
        .output()
        .map_err(|source| MdmError::Io {
            action: "convert policy",
            path: path.to_path_buf(),
            source,
        })?;
    if !output.status.success() {
        return Err(MdmError::Io {
            action: "convert policy",
            path: path.to_path_buf(),
            source: std::io::Error::other(String::from_utf8_lossy(&output.stderr).into_owned()),
        });
    }
    // JSON: a managed-preferences plist converted by plutil, shape owned by macOS.
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|source| MdmError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    Ok(Some(super::clear_plan::ObservedPolicy {
        keys: object.keys().cloned().collect(),
        api_key_fingerprint: object
            .get(crate::cowork_compat::POLICY_API_KEY)
            .and_then(serde_json::Value::as_str)
            .map(|v| crate::proxy::secret::fingerprint(v.trim())),
        base_url: object
            .get("inferenceGatewayBaseUrl")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        profile_managed: foreign_profile_installed(),
    }))
}

// Why: a managed-preferences file delivered by a configuration profile
// belongs to whoever installed the profile; any configuration profile other
// than the bridge's own, or a listing that cannot be read, keeps the file out
// of the stale-bridge takeover.
pub(crate) fn foreign_profile_installed() -> bool {
    let Ok(output) = std::process::Command::new("/usr/bin/profiles")
        .args(["list", "-type", "configuration"])
        .output()
    else {
        return true;
    };
    if !output.status.success() {
        return true;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split("profileIdentifier:").nth(1))
        .any(|id| id.trim() != PAYLOAD_IDENTIFIER)
}
