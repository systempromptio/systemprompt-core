//! Removal of the bridge's macOS configuration profile and managed
//! preferences, verified against the profile inventory and the files on disk.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]

use super::MdmError;
use super::macos::{MANAGED_PREFS_PATH, PAYLOAD_IDENTIFIER, bridge_prefs_path};

pub(crate) fn remove_profile() -> Result<bool, MdmError> {
    let cli_removed = !super::claude_code_settings::remove_all()?.is_empty();
    let user = std::env::var("USER").map_err(|e| MdmError::InvalidConfig(format!("USER: {e}")))?;
    if user.is_empty() || user.contains('/') || user == "." || user == ".." {
        return Err(MdmError::InvalidConfig(
            "invalid managed-preferences user".into(),
        ));
    }
    let paths = [
        std::path::PathBuf::from(MANAGED_PREFS_PATH),
        std::path::PathBuf::from(format!(
            "/Library/Managed Preferences/{user}/com.anthropic.claudefordesktop.plist"
        )),
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
        "set -e\n/usr/bin/profiles show -type configuration -output stdout-xml > {xml}\n/usr/bin/plutil -convert json -o {before} {xml}\nchmod 0644 {before}\nif /usr/bin/profiles remove -identifier {PAYLOAD_IDENTIFIER}; then removal_status=0; else removal_status=$?; fi\n/usr/bin/profiles show -type configuration -output stdout-xml > {xml}\n/usr/bin/plutil -convert json -o {listing} {xml}\nchmod 0644 {listing}\n",
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
    let profiles: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|source| MdmError::Json {
            path: listing,
            source,
        })?;
    if contains_profile_identifier(&profiles) {
        return Err(MdmError::InvalidConfig(format!(
            "configuration profile {PAYLOAD_IDENTIFIER} remains installed; remove it through System Settings or MDM"
        )));
    }
    if existing.is_empty() {
        return Ok(was_installed || cli_removed);
    }
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
            return Err(MdmError::InvalidConfig(format!(
                "{} remains after policy removal",
                path.display()
            )));
        }
    }
    Ok(true)
}

fn contains_profile_identifier(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(value) => value == PAYLOAD_IDENTIFIER,
        serde_json::Value::Array(values) => values.iter().any(contains_profile_identifier),
        serde_json::Value::Object(values) => values.values().any(contains_profile_identifier),
        _ => false,
    }
}
