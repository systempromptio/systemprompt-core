#![cfg(target_os = "windows")]

use std::io::Write;

use super::shared::{
    DESKTOP_DOMAIN, DomainRead, KEYS_OF_INTEREST, ProfileGenInputs, make_uuids,
    redact_if_sensitive, unique_stem,
};
use crate::config::store::managed_policy_store;
use crate::integration::host_app::GeneratedProfile;
use crate::winproc;

pub(super) fn read_domain(domain: &str) -> DomainRead {
    let mut out = DomainRead::default();

    if domain != DESKTOP_DOMAIN {
        return out;
    }

    let read = match managed_policy_store().read_managed_policy_keys(KEYS_OF_INTEREST) {
        Ok(r) => r,
        Err(_) => return out,
    };
    if read.values.is_empty() {
        return out;
    }
    out.source_path = read.source;
    for (name, value) in read.values {
        out.keys
            .insert(name.clone(), redact_if_sensitive(&name, value));
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
    let path = dir.join(format!("claude-bridge-{}.reg", unique_stem()));

    let body = super::reg_profile::render_reg(winproc::is_elevated(), inputs);
    std::fs::File::create(&path)?.write_all(body.as_bytes())?;

    Ok(GeneratedProfile {
        path: path.display().to_string(),
        bytes: body.len(),
        payload_uuid,
        profile_uuid,
    })
}

pub(super) fn install_profile(path: &str) -> std::io::Result<()> {
    let elevated = winproc::is_elevated();
    tracing::info!(path, elevated, "installing Claude Desktop profile");
    let body = std::fs::read_to_string(path)?;
    let entries = super::reg_profile::parse_reg_entries(&body);
    tracing::info!(
        path,
        parsed_values = entries.len(),
        names = ?entries.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        "parsed staged registry profile"
    );
    if entries.is_empty() {
        return Err(std::io::Error::other(
            "staged registry profile contained no policy values",
        ));
    }
    crate::config::store::write_managed_claude_policy(elevated, &entries).map_err(|e| {
        tracing::error!(error = %e, path, "managed Claude policy write failed");
        std::io::Error::other(e.to_string())
    })?;
    tracing::info!(
        value_count = entries.len(),
        "Claude Desktop profile installed"
    );
    Ok(())
}
