//! Filling schema-2 skill bodies from the synced plugin tree.
//!
//! A schema-2 manifest omits a plugin-delivered skill's `instructions` and
//! names the plugin file that carries them instead, so the manifest stays
//! small. Before any host receives skills, each such body is read back from
//! the org-plugins copy — whose files were already verified against the
//! manifest — and checked against the skill's own signed `sha256` (pass-through
//! frontmatter followed by the body). A body that does not match is never
//! guessed at: the skill is withheld from this run and reported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use systemprompt_models::bridge::manifest::skill_frontmatter::render_passthrough_frontmatter;

use super::safe_path::join_under;
use crate::gateway::manifest::{SignedManifest, SkillEntry};
use crate::hash::sha256_hex;

pub(super) fn resolve(manifest: &mut SignedManifest, org_plugins_root: &Path) {
    manifest.skills.retain_mut(|skill| {
        if skill.instructions.is_some() {
            return true;
        }
        let Some(source) = skill.source.as_ref() else {
            return true;
        };
        match body_from_plugin(skill, &org_plugins_root.join(source.plugin_id.as_str()), &source.path) {
            Ok(body) => {
                skill.instructions = Some(body);
                true
            },
            Err(reason) => {
                tracing::warn!(
                    target: "bridge::sync",
                    skill = %skill.id,
                    plugin_id = %source.plugin_id,
                    path = %source.path,
                    reason,
                    "skill body could not be taken from its plugin file; skill withheld this run"
                );
                false
            },
        }
    });
}

pub fn body_from_plugin(skill: &SkillEntry, plugin_dir: &Path, path: &str) -> Result<String, String> {
    let file = join_under(plugin_dir, path).map_err(|e| e.to_string())?;
    let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {}: {e}", file.display()))?;
    let body = strip_frontmatter(&raw);
    let rendered =
        render_passthrough_frontmatter(skill.frontmatter.as_ref()).map_err(|e| e.to_string())?;
    let actual = sha256_hex(format!("{rendered}{body}").as_bytes());
    if actual == skill.sha256.as_str() {
        Ok(body)
    } else {
        Err(format!(
            "digest {actual} does not match the signed {}",
            skill.sha256.as_str()
        ))
    }
}

// Why: mirrors systemprompt-manifest's line-anchored `strip_frontmatter`,
// which the gateway signs skill bodies with; the bridge does not link that crate.
fn strip_frontmatter(raw: &str) -> String {
    let content = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut lines = content.split_inclusive('\n');
    let Some(opening) = lines.next() else {
        return raw.to_owned();
    };
    if opening.trim_end() != "---" {
        return raw.to_owned();
    }
    let mut offset = opening.len();
    for line in lines {
        if line.trim_end() == "---" {
            return content[offset + line.len()..].trim().to_owned();
        }
        offset += line.len();
    }
    raw.to_owned()
}
