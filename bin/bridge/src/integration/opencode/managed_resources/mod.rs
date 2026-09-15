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
//! The governance-owning plugin also gets an `OpenCode` plugin module that
//! reports skill use through the loopback proxy, so `OpenCode` sessions land
//! in the same invocation ledger as Claude Code sessions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;

use crate::host_sync::{ApplyError, HostSync, HostSyncCtx};
use crate::integration::managed_skills::{SkillDirPolicy, SkillTarget};

mod config_json;
mod plugin_js;

use config_json::write_mcp_blocks;
pub(super) use plugin_js::remove_hook_plugin;
use plugin_js::write_hook_plugin;

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
            write_mcp_blocks(ctx.loopback, &ctx.manifest.managed_mcp_servers)?;
            write_hook_plugin(ctx.loopback, ctx.manifest)?;
        } else {
            skills().clear()?;
            write_mcp_blocks(ctx.loopback, &[])?;
            remove_hook_plugin()?;
        }
        Ok(())
    }

    fn clear(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        skills().clear()?;
        write_mcp_blocks(ctx.loopback, &[])?;
        remove_hook_plugin()?;
        Ok(())
    }
}
