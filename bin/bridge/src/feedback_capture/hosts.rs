//! Per-host skill roots for feedback capture.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FeedbackError, Result};
use crate::feedback::ReadbackFault;
use crate::gateway::manifest::SkillEntry;
use crate::host_sync::HostSyncCtx;
use std::path::PathBuf;

pub(super) fn roots(host: &str, ctx: &HostSyncCtx<'_>, skill: &SkillEntry) -> Result<Vec<PathBuf>> {
    if !crate::hash::safe_id_segment(skill.id.as_str()) {
        return Err(FeedbackError::Readback(ReadbackFault::UnsafeSkillId));
    }
    let roots = match host {
        "hermes" => vec![crate::integration::hermes::feedback_skill_root().join(skill.id.as_str())],
        "opencode" => vec![crate::integration::opencode::feedback_skill_root().join(
            crate::integration::managed_skills::kebab_dir(skill.id.as_str()),
        )],
        "codex-cli" => {
            crate::integration::codex_cli::feedback_skill_roots(ctx.loopback, ctx.manifest, skill)
        },
        "claude-code" => {
            crate::integration::claude_code_cli::feedback_skill_roots(ctx.manifest, skill)
        },
        "claude-desktop" => {
            if crate::integration::cowork_plugins::resolve_target()
                .map_err(|error| FeedbackError::Io(std::io::Error::other(error)))?
                .is_none()
            {
                return Ok(Vec::new());
            }
            skill
                .plugins
                .iter()
                .map(|plugin| {
                    ctx.org_plugins_root
                        .join(plugin.as_str())
                        .join("skills")
                        .join(skill.id.as_str().replace('_', "-"))
                })
                .collect()
        },
        _ => return Err(FeedbackError::Scope),
    };
    if roots.iter().any(|root| !root.join("SKILL.md").is_file()) {
        return Err(FeedbackError::Readback(ReadbackFault::SkillMissing));
    }
    Ok(roots)
}
