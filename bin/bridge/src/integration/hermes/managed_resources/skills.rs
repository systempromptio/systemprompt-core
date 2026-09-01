//! Hermes reads `SKILL.md` folders from `HERMES_HOME/skills/` verbatim, so the
//! shared managed-skills writer publishes them under the skill id unchanged.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::gateway::manifest::SignedManifest;
use crate::integration::managed_skills::{SkillDirPolicy, SkillTarget};
use crate::sync::ApplyError;

use super::super::config::skills_dir;

fn target() -> SkillTarget {
    SkillTarget {
        root: skills_dir(),
        host_id: "hermes",
        policy: SkillDirPolicy::Verbatim,
    }
}

pub(super) fn apply_skills(manifest: &SignedManifest) -> Result<(), ApplyError> {
    target().apply(manifest)
}

pub(super) fn clear_skills() -> Result<(), ApplyError> {
    target().clear()
}
