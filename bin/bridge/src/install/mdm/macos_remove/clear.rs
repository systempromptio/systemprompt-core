//! Clearing the bridge's Claude Desktop managed preferences and detecting
//! configuration profiles installed by someone other than the bridge.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::MdmError;
use super::super::macos::{MANAGED_PREFS_PATH, PAYLOAD_IDENTIFIER};
use super::{unprivileged_inventory_is_clean, user_prefs_path};

// Why: Claude Desktop reads both the per-user and the machine managed plist,
// and only an administrator can change either; one privileged script edits
// every file the bridge wrote and restarts cfprefsd once.
pub(crate) fn clear_bridge_preferences(
    gate: &crate::elevation_gate::ElevationGate,
    ours: Option<&str>,
) -> Result<Vec<String>, MdmError> {
    use super::super::clear_plan::{ClearPlan, plan_clear};

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
        gate,
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
) -> Result<Option<super::super::clear_plan::ObservedPolicy>, MdmError> {
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
    Ok(Some(super::super::clear_plan::ObservedPolicy {
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
