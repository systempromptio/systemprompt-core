#![cfg(target_os = "windows")]

use std::io::Write;

use super::shared::{
    DESKTOP_DOMAIN, DomainRead, KEYS_OF_INTEREST, ProfileGenInputs, make_uuids, now_unix,
    redact_if_sensitive,
};
use crate::integration::host_app::GeneratedProfile;
use crate::winproc;

const POLICY_KEY: &str = r"SOFTWARE\Policies\Claude";

pub(super) fn read_domain(domain: &str) -> DomainRead {
    let mut out = DomainRead::default();

    if domain != DESKTOP_DOMAIN {
        return out;
    }

    for hive in ["HKLM", "HKCU"] {
        let full = format!(r"{hive}\{POLICY_KEY}");
        let dump = match query_key(&full) {
            Some(d) if !d.is_empty() => d,
            _ => continue,
        };
        out.source_path = Some(full);
        for (name, value) in dump {
            if let Some(canonical) = canonical_key_name(&name) {
                out.keys
                    .insert(canonical.to_string(), redact_if_sensitive(canonical, value));
            }
        }
        break;
    }

    out
}

pub(super) fn list_claude_processes() -> Vec<String> {
    let mut hits: Vec<String> = crate::sysproc::list_processes()
        .into_iter()
        .filter_map(|p| {
            let lower = p.name.to_ascii_lowercase();
            let is_claude = lower == "claude.exe" || lower.starts_with("claude helper");
            let is_code = lower.contains("claude code") || lower == "claude-code.exe";
            if is_claude && !is_code {
                Some(p.name)
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
    let dir = std::env::temp_dir().join("systemprompt-bridge");
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = make_uuids();
    let path = dir.join(format!("claude-bridge-{}.reg", now_unix()));

    let body = render_reg(inputs);
    std::fs::File::create(&path)?.write_all(body.as_bytes())?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: body.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(path: &str) -> std::io::Result<()> {
    let status = winproc::reg_command().args(["import", path]).status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "reg import exited with {}",
            status.code().unwrap_or(-1)
        )));
    }
    Ok(())
}

fn query_key(full: &str) -> Option<Vec<(String, String)>> {
    let output = winproc::reg_command().args(["query", full]).output().ok()?;
    if !output.status.success() {
        return Some(Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut values = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("HKEY_") {
            continue;
        }
        if let Some((name, value)) = super::win_reg_parser::parse_reg_line(trimmed) {
            values.push((name, value));
        }
    }
    Some(values)
}

fn canonical_key_name(name: &str) -> Option<&'static str> {
    KEYS_OF_INTEREST
        .iter()
        .copied()
        .find(|k| k.eq_ignore_ascii_case(name))
}

fn render_reg(inputs: &ProfileGenInputs) -> String {
    let hive = if winproc::is_elevated() {
        "HKEY_LOCAL_MACHINE"
    } else {
        "HKEY_CURRENT_USER"
    };
    let mut out = String::new();
    out.push_str("Windows Registry Editor Version 5.00\r\n\r\n");
    out.push_str(&format!("[{hive}\\SOFTWARE\\Policies\\Claude]\r\n"));
    out.push_str("\"inferenceProvider\"=\"gateway\"\r\n");
    out.push_str(&format!(
        "\"inferenceGatewayBaseUrl\"=\"{}\"\r\n",
        reg_escape(&inputs.gateway_base_url)
    ));
    out.push_str(&format!(
        "\"inferenceGatewayApiKey\"=\"{}\"\r\n",
        reg_escape(&inputs.api_key)
    ));
    out.push_str("\"inferenceGatewayAuthScheme\"=\"bearer\"\r\n");
    let models: Vec<String> = if inputs.models.is_empty() {
        super::shared::default_models()
    } else {
        inputs.models.clone()
    };
    let models_json = serde_json::to_string(&models).unwrap_or_else(|_| "[]".into());
    out.push_str(&format!(
        "\"inferenceModels\"=\"{}\"\r\n",
        reg_escape(&models_json)
    ));
    if let Some(uuid) = inputs.organization_uuid.as_deref()
        && !uuid.is_empty()
    {
        out.push_str(&format!(
            "\"deploymentOrganizationUuid\"=\"{}\"\r\n",
            reg_escape(uuid)
        ));
    }
    out
}

fn reg_escape(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"")
}
