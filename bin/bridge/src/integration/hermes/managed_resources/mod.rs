//! Hermes CLI sync emitter.
//!
//! Hermes discovers HTTP MCP servers from `config.yaml` `mcp_servers.<name>`
//! and skills from `<HERMES_HOME>/skills/` (registered as an external dir), so
//! — unlike Codex — there is no local marketplace to stage: MCP connectors
//! merge straight into `config.yaml`, and skills are written directly into the
//! user skills dir with a sidecar tracking the ones the bridge manages.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use async_trait::async_trait;

use crate::host_sync::{ApplyError, HostSync, HostSyncCtx};

mod config_yaml;
mod skills;

use config_yaml::write_config_blocks;
use skills::{apply_skills, clear_skills};

#[derive(Clone, Copy, Debug)]
pub struct HermesSync;

#[async_trait]
impl HostSync for HermesSync {
    fn host_id(&self) -> &'static str {
        "hermes"
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        let has_content =
            !ctx.manifest.skills.is_empty() || !ctx.manifest.managed_mcp_servers.is_empty();
        if has_content {
            apply_skills(ctx.manifest)?;
            write_config_blocks(ctx.loopback, true, &ctx.manifest.managed_mcp_servers)?;
        } else {
            clear_skills()?;
            write_config_blocks(ctx.loopback, false, &[])?;
        }
        Ok(())
    }

    fn clear(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        clear_skills()?;
        write_config_blocks(ctx.loopback, false, &[])?;
        Ok(())
    }
}

fn io_err(context: &str, path: &Path, source: std::io::Error) -> ApplyError {
    ApplyError::Io {
        context: format!("{context} {}", path.display()),
        source,
    }
}
