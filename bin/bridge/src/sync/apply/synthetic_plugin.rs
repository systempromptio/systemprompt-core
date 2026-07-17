//! Writes the org's managed skills, agents, and hooks into the synthetic Cowork
//! org-plugin.
//!
//! The write is idempotent against content, which Cowork's polling requires:
//! `version.json` carries a hash of the bundle (not the gateway's per-poll
//! `manifest_version`) and is written *last*, so it doubles as the completion
//! marker the next sync's skip check keys on. An unchanged bundle is therefore
//! left byte-for-byte alone — never removed and rewritten — so Cowork never
//! observes a missing or half-written plugin between polls. MCP servers are
//! deliberately excluded: they ride the `managedMcpServers` policy, and
//! bundling them here would collide and leave a ghost connector.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::hash::{safe_id_segment, sha256_hex};
use super::hooks::{ensure_plugin_json_hooks_field, write_hooks_json};
use crate::config::paths;
use crate::gateway::manifest::{AgentEntry, SignedManifest, SkillEntry};
use crate::sync::host_sync::{HostSync, HostSyncCtx};
use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};
use systemprompt_models::bridge::plugin_bundle::PluginManifest;

pub(crate) struct ClaudeCodePluginSync;

#[async_trait]
impl HostSync for ClaudeCodePluginSync {
    fn host_id(&self) -> &'static str {
        "claude-code"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), super::ApplyError> {
        write_synthetic_plugin(ctx.org_plugins_root, ctx.manifest)?;
        prune_stale_locations(ctx.org_plugins_root);
        Ok(())
    }

    fn clear(&self) -> Result<(), super::ApplyError> {
        let Some(location) = paths::org_plugins_effective() else {
            return Ok(());
        };
        let root = location.path.join(paths::SYNTHETIC_PLUGIN_NAME);
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| super::ApplyError::Io {
                context: format!("remove {}", root.display()),
                source: e,
            })?;
        }
        Ok(())
    }
}

pub const PLUGIN_INSTALLATION_PREFERENCE: &str = "required";

pub fn render_plugin_json(manifest_version: &str) -> Result<Vec<u8>, serde_json::Error> {
    let manifest = PluginManifest {
        name: paths::SYNTHETIC_PLUGIN_NAME.to_owned(),
        description: "Skills, agents, and MCP servers managed by your organization.".to_owned(),
        version: manifest_version.to_owned(),
        author: None,
        hooks: None,
        keywords: Vec::new(),
        installation_preference: Some(PLUGIN_INSTALLATION_PREFERENCE.to_owned()),
    };
    serde_json::to_vec_pretty(&manifest)
}

fn render_version_json(version: &str) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec_pretty(&serde_json::json!({ "version": version }))
}

fn bundle_version(manifest: &SignedManifest) -> String {
    let mut skills: Vec<&SkillEntry> = manifest.skills.iter().collect();
    skills.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
    let mut agents: Vec<&AgentEntry> = manifest.agents.iter().collect();
    agents.sort_by(|a, b| a.name.as_str().cmp(b.name.as_str()));

    let mut buf = String::new();
    for s in skills {
        buf.push_str(s.id.as_str());
        buf.push('\u{0}');
        buf.push_str(&skill_markdown(s));
        buf.push('\u{0}');
    }
    buf.push('\u{1}');
    for a in agents {
        buf.push_str(a.name.as_str());
        buf.push('\u{0}');
        buf.push_str(&agent_markdown(a));
        buf.push('\u{0}');
    }
    buf.push('\u{1}');
    // Why: hash the hook definitions, not the rendered hooks.json — the latter
    // carries a rotating proxy token that would change the hash every poll.
    buf.push_str(&format!("{:?}", manifest.hooks));
    sha256_hex(buf.as_bytes())
}

fn read_existing_version(root: &Path) -> Option<String> {
    let bytes = fs::read(root.join("version.json")).ok()?;
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    value.get("version")?.as_str().map(str::to_owned)
}

#[tracing::instrument(level = "debug", skip(manifest))]
pub fn write_synthetic_plugin(
    org_plugins_root: &Path,
    manifest: &SignedManifest,
) -> Result<(), super::ApplyError> {
    let root = org_plugins_root.join(paths::SYNTHETIC_PLUGIN_NAME);

    let has_content =
        !manifest.skills.is_empty() || !manifest.agents.is_empty() || !manifest.hooks.is_empty();

    if !has_content {
        if root.exists() {
            fs::remove_dir_all(&root).map_err(|e| super::ApplyError::Io {
                context: format!("remove {}", root.display()),
                source: e,
            })?;
        }
        return Ok(());
    }

    let version = bundle_version(manifest);

    if read_existing_version(&root).as_deref() == Some(version.as_str())
        && root.join(".claude-plugin").join("plugin.json").is_file()
    {
        return Ok(());
    }

    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| super::ApplyError::Io {
            context: format!("clear {}", root.display()),
            source: e,
        })?;
    }
    fs::create_dir_all(&root).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", root.display()),
        source: e,
    })?;

    write_plugin_json(&root, &version)?;

    for skill in &manifest.skills {
        write_skill(&root, skill)?;
    }

    for agent in &manifest.agents {
        write_agent(&root, agent)?;
    }

    let synthetic_id = systemprompt_identifiers::PluginId::new(paths::SYNTHETIC_PLUGIN_NAME);
    write_hooks_json(&synthetic_id, &root, &manifest.hooks)?;
    ensure_plugin_json_hooks_field(&root)?;

    write_version_json(&root, &version)?;

    Ok(())
}

fn prune_stale_locations(effective_root: &Path) {
    prune_stale_locations_in(&paths::all_known_org_plugins_roots(), effective_root);
}

pub fn prune_stale_locations_in(roots: &[PathBuf], effective_root: &Path) {
    for root in roots {
        if !paths_equal(root, effective_root) {
            remove_if_present(
                &root.join(paths::SYNTHETIC_PLUGIN_NAME),
                "stale org-plugin copy",
            );
        }
        for marker in paths::LEGACY_ORG_PLUGINS_METADATA {
            remove_if_present(&root.join(marker), "legacy bridge metadata dir");
        }
    }
}

fn paths_equal(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

fn remove_if_present(path: &Path, what: &str) {
    if !path.exists() {
        return;
    }
    match fs::remove_dir_all(path) {
        Ok(()) => tracing::info!(
            target: "bridge::sync",
            path = %path.display(),
            "pruned {what}"
        ),
        Err(e) => tracing::warn!(
            target: "bridge::sync",
            path = %path.display(),
            error = %e,
            "could not prune {what} (likely permissions); skipping"
        ),
    }
}

fn write_plugin_json(root: &Path, version: &str) -> Result<(), super::ApplyError> {
    let dir = root.join(".claude-plugin");
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let bytes = render_plugin_json(version).map_err(|e| super::ApplyError::Serialize {
        what: "plugin.json".into(),
        source: e,
    })?;
    let path = dir.join("plugin.json");
    fs::write(&path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn write_version_json(root: &Path, version: &str) -> Result<(), super::ApplyError> {
    let bytes = render_version_json(version).map_err(|e| super::ApplyError::Serialize {
        what: "version.json".into(),
        source: e,
    })?;
    let path = root.join("version.json");
    fs::write(&path, bytes).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn write_skill(root: &Path, skill: &SkillEntry) -> Result<(), super::ApplyError> {
    if !safe_id_segment(skill.id.as_str()) {
        return Err(super::ApplyError::UnsafeSkillId(skill.id.clone()));
    }
    let dir = root.join("skills").join(skill.id.as_str());
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let path = dir.join("SKILL.md");
    fs::write(&path, skill_markdown(skill)).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn skill_markdown(skill: &SkillEntry) -> String {
    let trimmed = skill.instructions.trim_start();
    if trimmed.starts_with("---") {
        return skill.instructions.clone();
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
    if !skill.instructions.ends_with('\n') {
        out.push('\n');
    }
    out
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

fn write_agent(root: &Path, agent: &AgentEntry) -> Result<(), super::ApplyError> {
    if !safe_id_segment(agent.name.as_str()) {
        return Err(super::ApplyError::UnsafeAgentName(agent.name.to_string()));
    }
    let dir = root.join("agents");
    fs::create_dir_all(&dir).map_err(|e| super::ApplyError::Io {
        context: format!("create {}", dir.display()),
        source: e,
    })?;
    let path: PathBuf = dir.join(format!("{}.md", agent.name));
    fs::write(&path, agent_markdown(agent)).map_err(|e| super::ApplyError::Io {
        context: format!("write {}", path.display()),
        source: e,
    })
}

fn agent_markdown(agent: &AgentEntry) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str(&format!("name: {}\n", agent.name.as_str()));
    out.push_str(&format!(
        "description: {}\n",
        yaml_scalar(&agent.description)
    ));
    if let Some(model) = &agent.model {
        out.push_str(&format!("model: {}\n", yaml_scalar(model)));
    }
    out.push_str("---\n\n");
    if let Some(prompt) = &agent.system_prompt {
        out.push_str(prompt);
        if !prompt.ends_with('\n') {
            out.push('\n');
        }
    } else {
        out.push_str(&format!(
            "# {}\n\n{}\n",
            agent.display_name, agent.description
        ));
    }
    out
}
