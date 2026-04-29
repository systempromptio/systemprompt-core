#![cfg(target_os = "windows")]

use std::io::Write;

use super::shared::{
    DESKTOP_DOMAIN, GeneratedProfile, KEYS_OF_INTEREST, ManagedDomain, ProfileGenInputs,
    make_uuids, now_unix, redact_if_sensitive,
};
use crate::winproc;

const POLICY_KEY: &str = r"SOFTWARE\Policies\Claude";

pub(super) fn read_domain(domain: &str, required: &[&str]) -> ManagedDomain {
    let mut out = ManagedDomain {
        domain: domain.to_string(),
        ..Default::default()
    };

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
        out.installed = !out.keys.is_empty();
        break;
    }

    out.missing_required = required
        .iter()
        .filter(|k| !out.keys.contains_key(**k))
        .map(|k| (*k).to_string())
        .collect();

    out
}

pub(super) fn list_claude_processes() -> Vec<String> {
    let output = match winproc::tasklist_command()
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut hits: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let name = line.split(',').next()?.trim().trim_matches('"');
            let lower = name.to_ascii_lowercase();
            let is_claude = lower == "claude.exe" || lower.starts_with("claude helper");
            let is_code = lower.contains("claude code") || lower == "claude-code.exe";
            if is_claude && !is_code {
                Some(name.to_string())
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
    let dir = std::env::temp_dir().join("systemprompt-cowork");
    std::fs::create_dir_all(&dir)?;
    let (payload_uuid, profile_uuid) = make_uuids();
    let path = dir.join(format!("claude-cowork-{}.reg", now_unix()));

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
        let line = line.trim();
        if line.is_empty() || line.starts_with("HKEY_") {
            continue;
        }
        if let Some((name, value)) = parse_reg_line(line) {
            values.push((name, value));
        }
    }
    Some(values)
}

fn parse_reg_line(line: &str) -> Option<(String, String)> {
    let mut parts = line
        .splitn(3, char::is_whitespace)
        .filter(|s| !s.is_empty());
    let name = parts.next()?.to_string();
    let kind = parts.next()?;
    let value = parts.next().unwrap_or("").trim().to_string();
    let value = if kind == "REG_DWORD" {
        value
            .strip_prefix("0x")
            .and_then(|hex| u64::from_str_radix(hex, 16).ok())
            .map(|n| n.to_string())
            .unwrap_or(value)
    } else {
        value
    };
    Some((name, value))
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
    if !inputs.models.is_empty() {
        let models = inputs.models.join(",");
        out.push_str(&format!(
            "\"inferenceModels\"=\"{}\"\r\n",
            reg_escape(&models)
        ));
    }
    if let Some(uuid) = inputs.organization_uuid.as_deref() {
        if !uuid.is_empty() {
            out.push_str(&format!(
                "\"deploymentOrganizationUuid\"=\"{}\"\r\n",
                reg_escape(uuid)
            ));
        }
    }
    out
}

fn reg_escape(s: &str) -> String {
    s.replace('\\', r"\\").replace('"', "\\\"")
}
