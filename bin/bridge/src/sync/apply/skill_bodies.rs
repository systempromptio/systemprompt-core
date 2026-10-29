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

use super::ApplyError;
use super::safe_path::join_under;
use crate::gateway::manifest::{SignedManifest, SkillEntry};
use crate::hash::sha256_hex;
use crate::ids::SkillId;
use crate::warn_once::WarnOnce;

pub(super) fn resolve(manifest: &mut SignedManifest, org_plugins_root: &Path, seen: &WarnOnce) {
    manifest.skills.retain_mut(|skill| {
        if skill.instructions.is_some() {
            return true;
        }
        let Some(source) = skill.source.as_ref() else {
            return true;
        };
        match body_from_plugin(
            skill,
            &org_plugins_root.join(source.plugin_id.as_str()),
            &source.path,
        ) {
            Ok(body) => {
                skill.instructions = Some(body);
                true
            },
            Err(reason) if first_report(seen, &skill.id, &reason) => {
                tracing::warn!(
                    target: "bridge::sync",
                    skill = %skill.id,
                    plugin_id = %source.plugin_id,
                    path = %source.path,
                    reason = %reason,
                    "skill body could not be taken from its plugin file; skill withheld this run"
                );
                false
            },
            Err(reason) => {
                tracing::debug!(
                    target: "bridge::sync",
                    skill = %skill.id,
                    reason = %reason,
                    "skill body still withheld; already reported"
                );
                false
            },
        }
    });
}

// Why: sync runs on a timer, and the same unrecoverable body would otherwise
// warn on every pass; a skill is reported once per distinct failure.
fn first_report(seen: &WarnOnce, skill_id: &SkillId, reason: &SkillBodyError) -> bool {
    seen.changed(&format!("sync.skill_body.{skill_id}"), &reason.to_string())
}

/// Why a schema-2 skill body could not be taken from its plugin file.
#[derive(Debug, thiserror::Error)]
pub enum SkillBodyError {
    #[error("unsafe source path")]
    Path(#[source] ApplyError),
    #[error("read {path}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("render pass-through frontmatter")]
    Frontmatter(#[source] serde_yaml::Error),
    #[error("digest {actual} does not match the signed {signed}")]
    DigestMismatch { actual: String, signed: String },
}

pub fn body_from_plugin(
    skill: &SkillEntry,
    plugin_dir: &Path,
    path: &str,
) -> Result<String, SkillBodyError> {
    let file = join_under(plugin_dir, path).map_err(SkillBodyError::Path)?;
    let raw = std::fs::read_to_string(&file).map_err(|source| SkillBodyError::Read {
        path: file.clone(),
        source,
    })?;
    let body = strip_frontmatter(&raw);
    let rendered = render_passthrough_frontmatter(skill.frontmatter.as_ref())
        .map_err(SkillBodyError::Frontmatter)?;
    let actual = sha256_hex(format!("{rendered}{body}").as_bytes());
    if actual == skill.sha256.as_str() {
        Ok(body)
    } else {
        Err(SkillBodyError::DigestMismatch {
            actual,
            signed: skill.sha256.as_str().to_owned(),
        })
    }
}

// Why: the gateway omits a body only when this exact rule reproduces it from
// its SKILL.md layout (frontmatter, `---`, blank line, body, one newline).
fn strip_frontmatter(raw: &str) -> String {
    const SEPARATOR: &str = "\n---\n\n";
    let body = raw
        .find(SEPARATOR)
        .map_or(raw, |at| &raw[at + SEPARATOR.len()..]);
    body.strip_suffix('\n').unwrap_or(body).to_owned()
}
