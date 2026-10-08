//! Skill selection, SKILL.md rendering, and the content hash Codex versions on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use systemprompt_models::bridge::host::HostKind;

use crate::gateway::manifest::{SignedManifest, SkillEntry, skill_targets_host};
use crate::hash::{safe_id_segment, sha256_hex};
use crate::host_sync::ApplyError;
use crate::integration::managed_skills::skill_markdown;

use super::io_err;

pub(super) fn targets_codex(skill: &SkillEntry) -> bool {
    skill_targets_host(skill, HostKind::CodexCli)
}

pub(super) fn bundle_version(
    loopback: &crate::proxy::LoopbackEndpoint,
    manifest: &SignedManifest,
) -> Result<String, ApplyError> {
    let mut skills: Vec<&SkillEntry> = manifest
        .skills
        .iter()
        .filter(|s| targets_codex(s))
        .collect();
    skills.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    let mut buf = String::new();
    for s in skills {
        buf.push_str(s.id.as_str());
        buf.push('\u{0}');
        buf.push_str(&skill_markdown(s)?);
        buf.push('\u{0}');
    }
    buf.push('\u{1}');

    let mut servers: Vec<(String, String)> = manifest
        .managed_mcp_servers
        .iter()
        .map(|s| {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            let url = loopback.mcp_url(&slug);
            (slug, url)
        })
        .collect();
    servers.sort();
    for (slug, url) in servers {
        buf.push_str(&slug);
        buf.push('\u{0}');
        buf.push_str(&url);
        buf.push('\u{0}');
    }

    Ok(sha256_hex(buf.as_bytes())[..16].to_owned())
}

pub(super) fn write_skill(plugin_dir: &Path, skill: &SkillEntry) -> Result<(), ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = plugin_dir.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| io_err("create skill dir", &dir, e))?;
    let path = dir.join("SKILL.md");
    crate::fsutil::atomic_write_0644(&path, skill_markdown(skill)?.as_bytes())
        .map_err(|e| io_err("write SKILL.md", &path, e))
}
