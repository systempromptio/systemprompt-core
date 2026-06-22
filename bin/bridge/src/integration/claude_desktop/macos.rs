#![cfg(target_os = "macos")]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::shared::{
    DomainRead, KEYS_OF_INTEREST, ProfileGenInputs, make_uuids, redact_if_sensitive, unique_stem,
};
use crate::install::xml::escape;
use crate::integration::host_app::GeneratedProfile;

const MANAGED_PREFS_ROOT: &str = "/Library/Managed Preferences";
const PROFILE_TMPL: &str = include_str!("templates/claude_desktop_profile.mobileconfig.tmpl");

pub(super) fn read_domain(domain: &str) -> DomainRead {
    let mut out = DomainRead::default();

    let plist_path = candidates(domain).into_iter().find(|p| p.exists());

    if let Some(path) = plist_path.as_ref() {
        out.source_path = Some(path.display().to_string());
    }

    let plist_json = plist_path
        .as_deref()
        .and_then(read_plist_as_json)
        .unwrap_or(serde_json::Value::Null);

    for key in KEYS_OF_INTEREST {
        if let Some(val) = read_key_value(&plist_json, domain, key) {
            out.keys.insert((*key).to_owned(), val);
        }
    }

    out
}

pub(super) fn list_claude_processes() -> Vec<String> {
    let mut hits: Vec<String> = crate::sysproc::list_processes()
        .into_iter()
        .filter_map(|p| {
            let name_lower = p.name.to_ascii_lowercase();
            let path_lower = p
                .path
                .as_deref()
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            let matches = path_lower.contains("/claude.app/")
                || path_lower.ends_with("/claude")
                || name_lower.contains("claude helper")
                || path_lower.contains("claude helper");
            let is_code = name_lower.contains("claude code") || path_lower.contains("claude code");
            if matches && !is_code {
                Some(if path_lower.is_empty() {
                    name_lower
                } else {
                    path_lower
                })
            } else {
                None
            }
        })
        .collect();
    hits.sort();
    hits.dedup();
    hits
}

pub(super) fn write_profile(inputs: &ProfileGenInputs) -> std::io::Result<GeneratedProfile> {
    let dir = std::env::temp_dir().join(crate::brand::brand().working_dir_name);
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = make_uuids();
    let path = dir.join(format!("claude-bridge-{}.mobileconfig", unique_stem()));

    let xml = render_profile(inputs, &payload_uuid, &profile_uuid);
    std::fs::File::create(&path)?.write_all(xml.as_bytes())?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: xml.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(path: &str) -> std::io::Result<()> {
    Command::new("/usr/bin/open").arg(path).status()?;
    Ok(())
}

fn candidates(domain: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(user) = std::env::var("USER") {
        if !user.is_empty() {
            out.push(
                PathBuf::from(MANAGED_PREFS_ROOT)
                    .join(&user)
                    .join(format!("{domain}.plist")),
            );
        }
    }
    out.push(PathBuf::from(MANAGED_PREFS_ROOT).join(format!("{domain}.plist")));
    out
}

fn read_plist_as_json(path: &Path) -> Option<serde_json::Value> {
    let output = Command::new("/usr/bin/plutil")
        .arg("-convert")
        .arg("json")
        .arg("-o")
        .arg("-")
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn read_key_value(plist_json: &serde_json::Value, _domain: &str, key: &str) -> Option<String> {
    if let Some(val) = plist_json.get(key) {
        return Some(format_plist_value(key, val));
    }

    let raw = crate::config::store::managed_policy_store()
        .read_managed_policy(key)
        .ok()
        .flatten()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(redact_if_sensitive(key, trimmed.to_owned()))
}

fn format_plist_value(key: &str, value: &serde_json::Value) -> String {
    let rendered = match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect::<Vec<_>>()
            .join(", "),
        other => other.to_string(),
    };
    redact_if_sensitive(key, rendered)
}

fn render_profile(inputs: &ProfileGenInputs, payload_uuid: &str, profile_uuid: &str) -> String {
    let models = if inputs.models.is_empty() {
        super::shared::default_models()
    } else {
        inputs.models.clone()
    };
    let models_xml: String = models
        .iter()
        .map(|m| format!("            <string>{}</string>", escape(m)))
        .collect::<Vec<_>>()
        .join("\n");

    PROFILE_TMPL
        .replace("{profile_uuid}", &escape(profile_uuid))
        .replace("{payload_uuid}", &escape(payload_uuid))
        .replace("{base_url}", &escape(&inputs.gateway_base_url))
        .replace("{api_key}", &escape(&inputs.api_key))
        .replace("{models_xml}", &models_xml)
        .replace("{headers_xml}", &render_headers_xml(&inputs.headers))
}

fn render_headers_xml(headers: &std::collections::BTreeMap<String, String>) -> String {
    if headers.is_empty() {
        return String::new();
    }
    let mut out = String::from("        <key>inferenceCustomHeaders</key>\n        <dict>\n");
    for (name, value) in headers {
        out.push_str(&format!(
            "          <key>{}</key>\n          <string>{}</string>\n",
            escape(name),
            escape(value)
        ));
    }
    out.push_str("        </dict>\n");
    out
}
