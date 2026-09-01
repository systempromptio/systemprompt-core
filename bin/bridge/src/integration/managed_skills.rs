//! Managed skills the bridge publishes straight into a host's user skills
//! directory, shared by every host that reads `SKILL.md` folders directly
//! (Hermes, `OpenCode`) rather than through a marketplace.
//!
//! The directory is shared with skills the user authors, so the bridge records
//! the directories it manages in a sidecar and prunes only those — a stale
//! managed skill is removed, but a user's own skill is never touched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::gateway::manifest::{SignedManifest, SkillEntry};
use crate::sync::{ApplyError, safe_id_segment, sha256_hex};

const SIDECAR: &str = ".systemprompt-managed.json";

// Why: `Verbatim` keeps the id as the directory and passes upstream front
// matter through; `KebabNamed` kebab-cases the id and forces the front matter
// `name` to match, for hosts that reject a skill whose name differs from its
// folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkillDirPolicy {
    Verbatim,
    KebabNamed,
}

#[derive(Debug, Clone)]
pub(crate) struct SkillTarget {
    pub root: PathBuf,
    pub host_id: &'static str,
    pub policy: SkillDirPolicy,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ManagedState {
    version: String,
    ids: Vec<String>,
}

impl SkillTarget {
    // Why: skills may target specific hosts; an empty list means every host.
    // Skip skills aimed elsewhere so a Cowork-only skill never lands here.
    fn targets(&self, skill: &SkillEntry) -> bool {
        skill.hosts.is_empty() || skill.hosts.iter().any(|h| h == self.host_id)
    }

    fn dir_name(&self, skill: &SkillEntry) -> String {
        match self.policy {
            SkillDirPolicy::Verbatim => skill.id.as_str().to_owned(),
            SkillDirPolicy::KebabNamed => kebab_dir(skill.id.as_str()),
        }
    }

    fn render(&self, skill: &SkillEntry, dir: &str) -> String {
        match self.policy {
            SkillDirPolicy::Verbatim => skill_markdown(skill),
            SkillDirPolicy::KebabNamed => skill_markdown_named(skill, dir),
        }
    }

    fn selected<'m>(&self, manifest: &'m SignedManifest) -> Result<Vec<Selected<'m>>, ApplyError> {
        let mut by_dir: BTreeMap<String, &SkillEntry> = BTreeMap::new();
        let mut out = Vec::new();
        for skill in manifest.skills.iter().filter(|s| self.targets(s)) {
            if !safe_id_segment(skill.id.as_str()) {
                return Err(ApplyError::UnsafeSkillId(skill.id.clone()));
            }
            let dir = self.dir_name(skill);
            if let Some(prior) = by_dir.insert(dir.clone(), skill) {
                return Err(ApplyError::SkillDirCollision {
                    dir,
                    first: prior.id.as_str().to_owned(),
                    second: skill.id.as_str().to_owned(),
                });
            }
            out.push(Selected { skill, dir });
        }
        out.sort_by(|a, b| a.dir.cmp(&b.dir));
        Ok(out)
    }

    pub(crate) fn bundle_version(&self, manifest: &SignedManifest) -> Result<String, ApplyError> {
        let mut buf = String::new();
        for s in self.selected(manifest)? {
            buf.push_str(&s.dir);
            buf.push('\u{0}');
            buf.push_str(&self.render(s.skill, &s.dir));
            buf.push('\u{0}');
        }
        Ok(sha256_hex(buf.as_bytes())[..16].to_owned())
    }

    pub(crate) fn apply(&self, manifest: &SignedManifest) -> Result<(), ApplyError> {
        let selected = self.selected(manifest)?;
        let version = self.bundle_version(manifest)?;
        let previous = read_state(&self.root);

        let mut new_ids: Vec<String> = Vec::with_capacity(selected.len());
        for s in &selected {
            write_skill(&self.root, &s.dir, &self.render(s.skill, &s.dir))?;
            new_ids.push(s.dir.clone());
        }

        // Why: prune managed dirs we wrote before but no longer manage; never
        // touch a dir the sidecar did not claim.
        for stale in previous.ids.iter().filter(|id| !new_ids.contains(id)) {
            let dir = self.root.join(stale);
            if dir.exists() {
                fs::remove_dir_all(&dir).map_err(|e| io_err("prune stale skill", &dir, e))?;
            }
        }

        write_state(
            &self.root,
            &ManagedState {
                version,
                ids: new_ids,
            },
        )
    }

    pub(crate) fn clear(&self) -> Result<(), ApplyError> {
        let previous = read_state(&self.root);
        for id in &previous.ids {
            let dir = self.root.join(id);
            if dir.exists() {
                fs::remove_dir_all(&dir).map_err(|e| io_err("remove managed skill", &dir, e))?;
            }
        }
        let sidecar = self.root.join(SIDECAR);
        if sidecar.exists() {
            fs::remove_file(&sidecar).map_err(|e| io_err("remove skills sidecar", &sidecar, e))?;
        }
        Ok(())
    }
}

struct Selected<'m> {
    skill: &'m SkillEntry,
    dir: String,
}

// Why: lowercase `[a-z0-9-]`, no leading/trailing/doubled dashes, at most 64
// chars — the strictest folder rule among the hosts, so one mapping serves all.
pub(crate) fn kebab_dir(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    let mut last_dash = true;
    for c in id.chars() {
        let mapped = if c.is_ascii_alphanumeric() {
            Some(c.to_ascii_lowercase())
        } else if last_dash {
            None
        } else {
            Some('-')
        };
        if let Some(m) = mapped {
            last_dash = m == '-';
            out.push(m);
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out.truncate(64);
    while out.ends_with('-') {
        out.pop();
    }
    out
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
        what: "managed skills sidecar".into(),
        source: e,
    })?;
    let path = root.join(SIDECAR);
    fs::write(&path, bytes).map_err(|e| io_err("write skills sidecar", &path, e))
}

fn write_skill(root: &Path, dir_name: &str, content: &str) -> Result<(), ApplyError> {
    let dir = root.join(dir_name);
    fs::create_dir_all(&dir).map_err(|e| io_err("create skill dir", &dir, e))?;
    let path = dir.join("SKILL.md");
    // Why: skip an identical write so the file stays byte-stable and the host
    // never sees a spurious mtime change.
    if let Ok(existing) = fs::read_to_string(&path)
        && existing == content
    {
        return Ok(());
    }
    fs::write(&path, content).map_err(|e| io_err("write SKILL.md", &path, e))
}

pub(crate) fn skill_markdown(skill: &SkillEntry) -> String {
    let trimmed = skill.instructions.trim_start();
    if trimmed.starts_with("---") {
        return ensure_trailing_newline(skill.instructions.clone());
    }
    ensure_trailing_newline(
        front_matter(skill.name.as_str(), &skill.description) + &skill.instructions,
    )
}

// Why: the host refuses a skill whose front matter `name` differs from its
// folder, so an upstream `name:` line is replaced rather than trusted.
fn skill_markdown_named(skill: &SkillEntry, dir: &str) -> String {
    let trimmed = skill.instructions.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---")
        && let Some(end) = rest.find("\n---")
    {
        let block = &rest[..end];
        let tail = &rest[end + "\n---".len()..];
        let mut lines: Vec<String> = block
            .lines()
            .filter(|l| !l.trim_start().starts_with("name:"))
            .map(str::to_owned)
            .collect();
        lines.retain(|l| !l.trim().is_empty());
        lines.insert(0, format!("name: {dir}"));
        return ensure_trailing_newline(format!("---\n{}\n---{tail}", lines.join("\n")));
    }
    ensure_trailing_newline(front_matter(dir, &skill.description) + &skill.instructions)
}

fn front_matter(name: &str, description: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: {}\n---\n\n",
        yaml_scalar(description)
    )
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

fn io_err(context: &str, path: &Path, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: format!("{context} {}", path.display()),
        source,
    }
}
