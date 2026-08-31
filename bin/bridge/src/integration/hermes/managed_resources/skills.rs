//! Skill selection, SKILL.md rendering, and safe pruning of the managed skills
//! the bridge writes into the Hermes user skills dir.
//!
//! The user skills dir is shared with skills the user authors, so the bridge
//! records the ids it manages in a sidecar and prunes only those — a stale
//! managed skill is removed, but a user's own skill is never touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::gateway::manifest::{SignedManifest, SkillEntry};
use crate::sync::{ApplyError, safe_id_segment, sha256_hex};

use super::super::config::skills_dir;
use super::io_err;

const SIDECAR: &str = ".systemprompt-managed.json";

#[derive(Debug, Default, Serialize, Deserialize)]
struct ManagedState {
    version: String,
    ids: Vec<String>,
}

// Why: skills may target specific hosts; an empty list means every host. Skip
// skills aimed elsewhere so a Cowork-only skill never lands in Hermes.
pub(super) fn targets_hermes(skill: &SkillEntry) -> bool {
    skill.hosts.is_empty() || skill.hosts.iter().any(|h| h == "hermes")
}

pub(super) fn bundle_version(manifest: &SignedManifest) -> String {
    let mut skills: Vec<&SkillEntry> = manifest
        .skills
        .iter()
        .filter(|s| targets_hermes(s))
        .collect();
    skills.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));

    let mut buf = String::new();
    for s in skills {
        buf.push_str(s.id.as_str());
        buf.push('\u{0}');
        buf.push_str(&skill_markdown(s));
        buf.push('\u{0}');
    }
    sha256_hex(buf.as_bytes())[..16].to_owned()
}

pub(super) fn apply_skills(manifest: &SignedManifest) -> Result<(), ApplyError> {
    let root = skills_dir();
    let version = bundle_version(manifest);
    let previous = read_state(&root);

    let targeted: Vec<&SkillEntry> = manifest
        .skills
        .iter()
        .filter(|s| targets_hermes(s))
        .collect();

    let mut new_ids: Vec<String> = Vec::with_capacity(targeted.len());
    for skill in &targeted {
        if !safe_id_segment(skill.id.as_str()) {
            return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
        }
        write_skill(&root, skill)?;
        new_ids.push(skill.id.as_str().to_owned());
    }

    // Why: prune managed ids we wrote before but no longer manage; never touch a
    // dir the sidecar did not claim.
    for stale in previous.ids.iter().filter(|id| !new_ids.contains(id)) {
        let dir = root.join(stale);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| io_err("prune stale skill", &dir, e))?;
        }
    }

    write_state(
        &root,
        &ManagedState {
            version,
            ids: new_ids,
        },
    )
}

pub(super) fn clear_skills() -> Result<(), ApplyError> {
    let root = skills_dir();
    let previous = read_state(&root);
    for id in &previous.ids {
        let dir = root.join(id);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| io_err("remove managed skill", &dir, e))?;
        }
    }
    let sidecar = root.join(SIDECAR);
    if sidecar.exists() {
        fs::remove_file(&sidecar).map_err(|e| io_err("remove skills sidecar", &sidecar, e))?;
    }
    Ok(())
}

fn read_state(root: &Path) -> ManagedState {
    let Ok(bytes) = fs::read(root.join(SIDECAR)) else {
        return ManagedState::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn write_state(root: &Path, state: &ManagedState) -> Result<(), ApplyError> {
    fs::create_dir_all(root).map_err(|e| io_err("create skills dir", root, e))?;
    let bytes = serde_json::to_vec_pretty(state).map_err(|e| ApplyError::Serialize {
        what: "hermes managed skills sidecar".into(),
        source: e,
    })?;
    let path = root.join(SIDECAR);
    fs::write(&path, bytes).map_err(|e| io_err("write skills sidecar", &path, e))
}

fn write_skill(root: &Path, skill: &SkillEntry) -> Result<(), ApplyError> {
    let dir = root.join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| io_err("create skill dir", &dir, e))?;
    let path = dir.join("SKILL.md");
    let content = skill_markdown(skill);
    // Why: skip an identical write so the file stays byte-stable and Hermes
    // never sees a spurious mtime change.
    if let Ok(existing) = fs::read_to_string(&path)
        && existing == content
    {
        return Ok(());
    }
    fs::write(&path, content).map_err(|e| io_err("write SKILL.md", &path, e))
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
