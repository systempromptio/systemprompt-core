//! Shared Desktop fleet defaults and verified operator overrides.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::MdmError;
use super::policy::{PolicyEntry, PolicyInputs, PolicyValue};
pub use systemprompt_models::bridge::desktop_policy::{DesktopPolicy, settings_catalog};

pub fn resolved_settings(policy: &DesktopPolicy) -> Result<Vec<PolicyEntry>, MdmError> {
    policy.validate()?;
    let catalog = settings_catalog().map_err(|source| MdmError::ConfigJson {
        key: "desktop_policy",
        source,
    })?;
    let mut out = Vec::new();
    for setting in catalog.settings {
        if setting.key.starts_with("otlp") {
            continue;
        }
        let Some(value) = policy
            .settings
            .get(&setting.key)
            .cloned()
            .or(setting.default)
        else {
            continue;
        };
        let key: &'static str =
            setting_key(&setting.key).ok_or_else(|| MdmError::InvalidConfig {
                key: "desktop_policy",
                detail: format!("unregistered setting {}", setting.key),
            })?;
        let value = match value {
            serde_json::Value::Bool(value) => PolicyValue::Bool(value),
            serde_json::Value::String(value) => PolicyValue::Str(value),
            value if value.is_number() => PolicyValue::Str(value.to_string()),
            value => PolicyValue::Json(value),
        };
        out.push((key, value));
    }
    Ok(out)
}

pub fn verified_operator_policy() -> Result<DesktopPolicy, MdmError> {
    let Some(fragment) = crate::mcp_registry::read_envelope().map_err(|source| MdmError::Io {
        action: "read signed Desktop policy",
        path: crate::config::paths::bridge_metadata_dir().unwrap_or_default(),
        source,
    })?
    else {
        return Ok(DesktopPolicy::default());
    };
    let cfg = crate::config::load()?;
    if crate::config::GatewayIdentity::new(&fragment.gateway)?
        != crate::config::GatewayIdentity::new(&crate::config::gateway_url_or_default(&cfg))?
    {
        return Ok(DesktopPolicy::default());
    }
    let Some(key) = crate::config::pinned_pubkey()? else {
        return Err(MdmError::InvalidConfig {
            key: "desktop_policy",
            detail: "signed manifest has no gateway-bound signing key".to_owned(),
        });
    };
    crate::gateway::manifest::verify_envelope(&fragment.envelope, key.as_str())?;
    let manifest = crate::gateway::manifest::decode_payload(&fragment.envelope)?;
    Ok(manifest.desktop_policy)
}

pub fn entries(inputs: &PolicyInputs<'_>) -> Result<Vec<PolicyEntry>, MdmError> {
    let policy = verified_operator_policy()?;
    entries_with_policy(inputs, &policy)
}

pub fn entries_with_policy(
    inputs: &PolicyInputs<'_>,
    policy: &DesktopPolicy,
) -> Result<Vec<PolicyEntry>, MdmError> {
    let mut entries = resolved_settings(policy)?;
    let endpoint = format!("{}/otel", inputs.base_url.trim_end_matches('/'));
    entries.extend([
        ("otlpEndpoint", PolicyValue::Str(endpoint)),
        ("otlpProtocol", PolicyValue::Str("http/protobuf".to_owned())),
        (
            "otlpAuthMode",
            PolicyValue::Str("inference-credential".to_owned()),
        ),
        (
            "otlpDesktopLogLevel",
            PolicyValue::Str(
                policy
                    .settings
                    .get("otlpDesktopLogLevel")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("info")
                    .to_owned(),
            ),
        ),
        (
            "otlpTracesEnabled",
            PolicyValue::Bool(
                policy
                    .settings
                    .get("otlpTracesEnabled")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true),
            ),
        ),
        (
            "otlpContentCapture",
            PolicyValue::Json(serde_json::json!([])),
        ),
    ]);
    Ok(entries)
}

pub fn setting_key(key: &str) -> Option<&'static str> {
    SETTING_KEYS
        .iter()
        .copied()
        .find(|candidate| *candidate == key)
}

pub use super::desktop_settings::SETTING_KEYS;

pub fn supports_version(installed: &str, required: &str) -> bool {
    fn parts(value: &str) -> Option<Vec<u64>> {
        match value
            .trim()
            .split('.')
            .map(str::parse)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(parts) => Some(parts),
            Err(error) => {
                tracing::warn!(%error, version = value, "invalid Desktop version");
                None
            },
        }
    }
    match (parts(installed), parts(required)) {
        (Some(mut actual), Some(mut minimum)) => {
            let length = actual.len().max(minimum.len());
            actual.resize(length, 0);
            minimum.resize(length, 0);
            actual >= minimum
        },
        _ => false,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn installed_desktop_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        for app in [
            std::path::PathBuf::from("/Applications/Claude.app"),
            dirs::home_dir()?.join("Applications/Claude.app"),
        ] {
            let output = std::process::Command::new("/usr/libexec/PlistBuddy")
                .args(["-c", "Print :CFBundleShortVersionString"])
                .arg(app.join("Contents/Info.plist"))
                .output()
                .inspect_err(|error| tracing::warn!(%error, "could not inspect Desktop version"))
                .ok()?;
            if output.status.success() {
                let version = String::from_utf8(output.stdout)
                    .inspect_err(|error| tracing::warn!(%error, "invalid Desktop version encoding"))
                    .ok()?
                    .trim()
                    .to_owned();
                if !version.is_empty() {
                    return Some(version);
                }
            }
        }
        None
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new("powershell.exe");
        crate::winproc::no_window(&mut command);
        let output = command.args([
            "-NoProfile", "-NonInteractive", "-Command",
            "(Get-AppxPackage | Where-Object { $_.Name -like '*Claude*' } | Select-Object -First 1).Version"
        ]).output().inspect_err(|error| tracing::warn!(%error, "could not inspect Desktop version")).ok()?;
        let version = String::from_utf8(output.stdout)
            .inspect_err(|error| tracing::warn!(%error, "invalid Desktop version encoding"))
            .ok()?
            .trim()
            .to_owned();
        (!version.is_empty() && output.status.success()).then_some(version)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const fn installed_desktop_version() -> Option<String> {
    None
}
