//! Skill selection, SKILL.md rendering, and the content hash Codex versions on.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use crate::gateway::manifest::{SignedManifest, SkillEntry};
use crate::sync::{ApplyError, safe_id_segment, sha256_hex};

use super::io_err;

// Why: skills may target specific hosts; an empty list means every host. The
// Codex surface must skip skills aimed elsewhere (e.g. hosts: [cowork]), or
// the Cowork setup skill shows up in a host that cannot run it.
pub(super) fn targets_codex(skill: &SkillEntry) -> bool {
    skill.hosts.is_empty() || skill.hosts.iter().any(|h| h == "codex" || h == "codex-cli")
}

pub(super) fn bundle_version(manifest: &SignedManifest) -> String {
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
        buf.push_str(&skill_markdown(s));
        buf.push('\u{0}');
    }
    buf.push('\u{1}');

    let mut servers: Vec<(String, String)> = manifest
        .managed_mcp_servers
        .iter()
        .map(|s| {
            let slug = crate::mcp_registry::normalize_key(s.name.as_str());
            let url = crate::proxy::mcp_url(&slug);
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

    sha256_hex(buf.as_bytes())[..16].to_owned()
}

pub(super) fn write_skill(plugin_dir: &Path, skill: &SkillEntry) -> Result<(), ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = plugin_dir.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| io_err("create skill dir", &dir, e))?;
    let path = dir.join("SKILL.md");
    fs::write(&path, skill_markdown(skill)).map_err(|e| io_err("write SKILL.md", &path, e))
}

fn skill_markdown(skill: &SkillEntry) -> String {
    let trimmed = skill.instructions.trim_start();
    if trimmed.starts_with("---") {
        return ensure_trailing_newline(skill.instructions.clone());
    }
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", skill.name.as_str()));
    out.push_str(&format!(
        "description: {}\n",
        yaml_scalar(&skill.description)
    ));
    out.push_str("---\n\n");
    out.push_str(&skill.instructions);
    ensure_trailing_newline(out)
}

fn ensure_trailing_newline(mut s: String) -> String {
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn yaml_scalar(s: &str) -> String {
    let needs_quotes = s.contains(':')
        || s.contains('#')
        || s.starts_with(['-', '?', '!', '&', '*', '|', '>', '\'', '"', '%', '@', '`']);
    if !needs_quotes {
        return s.to_owned();
    }
    let escaped = s.replace('"', "\\\"");
    format!("\"{escaped}\"")
}
