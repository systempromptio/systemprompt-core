//! `OpenCode` sync emitter.
//!
//! MCP connectors merge into the user's global `opencode.json` as remote
//! servers and skills are written into the user skills directory — both
//! user-owned, because scheduled sync runs unattended and can never prompt for
//! administrator rights. The managed tier carries only the provider block the
//! installer owns.
//!
//! `OpenCode` also reads `~/.claude/skills`; a user-authored skill there with
//! the same folder name dedupes against a managed one, which the bridge cannot
//! prevent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;

use crate::integration::managed_skills::{SkillDirPolicy, SkillTarget};
use crate::sync::ApplyError;
use crate::sync::host_sync::{HostSync, HostSyncCtx};

mod config_json;

use config_json::write_mcp_blocks;

#[derive(Clone, Copy, Debug)]
pub struct OpenCodeSync;

fn skills() -> SkillTarget {
    SkillTarget {
        root: super::config::skills_dir(),
        host_id: "opencode",
        policy: SkillDirPolicy::KebabNamed,
    }
}

#[async_trait]
impl HostSync for OpenCodeSync {
    fn host_id(&self) -> &'static str {
        "opencode"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let has_content =
            !ctx.manifest.skills.is_empty() || !ctx.manifest.managed_mcp_servers.is_empty();
        if has_content {
            skills().apply(ctx.manifest)?;
            write_mcp_blocks(&ctx.manifest.managed_mcp_servers)?;
        } else {
            skills().clear()?;
            write_mcp_blocks(&[])?;
        }
        Ok(())
    }

    fn clear(&self) -> Result<(), ApplyError> {
        skills().clear()?;
        write_mcp_blocks(&[])?;
        Ok(())
    }
}
