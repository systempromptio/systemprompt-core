//! `OpenCode` sync emitter, shared by the desktop app and the CLI.
//!
//! Content is projected whichever tier owns the provider: managed skills into
//! the user skills directory, the rules block of `AGENTS.md`, bundle agents,
//! plugins and themes, the hooks plugin that reports skill use through the
//! loopback proxy, and marketplace client defaults. MCP connectors are
//! written as `mcp.servers` remote entries only while the provider is in the
//! user tier; under an admin-managed (MDM) provider the admin owns MCP and
//! policy config, and the bridge's own entries are removed. Everything is
//! user-owned, because scheduled sync runs unattended and can never prompt
//! for administrator rights.
//!
//! `OpenCode` also reads `~/.claude/skills`; a user-authored skill there with
//! the same folder name dedupes against a managed one, which the bridge cannot
//! prevent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;

use systemprompt_models::bridge::host::{HostKind, HostProduct};

use crate::host_sync::{ApplyError, HostSync, HostSyncCtx, HostSyncReport};
use crate::integration::managed_skills::{SkillDirPolicy, SkillTarget};

mod config_json;
mod content;
mod owned;
mod plugin_js;

use config_json::{apply_cli_defaults, select_default_theme, write_mcp_servers};
pub use content::{agents_block, as_subagent, splice_block};
pub(super) use plugin_js::remove_hook_plugin;
use plugin_js::write_hook_plugin;

use super::tier::{ProviderTier, provider_tier};

#[derive(Clone, Copy, Debug)]
pub struct OpenCodeSync;

fn skills() -> SkillTarget {
    SkillTarget {
        root: super::config::skills_dir(),
        product: HostProduct::OpenCode,
        policy: SkillDirPolicy::KebabNamed,
    }
}

pub(super) fn remove_owned_content() -> Result<(), ApplyError> {
    let root = super::config::user_dir();
    skills().clear()?;
    content::write_agents_md(&root, &[])?;
    owned::sync(&root, &std::collections::BTreeMap::new())?;
    remove_hook_plugin()
}

#[async_trait]
impl HostSync for OpenCodeSync {
    fn host_id(&self) -> HostKind {
        super::TOKEN_HOST
    }

    fn serves(&self, host: HostKind) -> bool {
        host.product() == HostProduct::OpenCode
    }

    async fn apply(&self, ctx: &HostSyncCtx<'_>) -> Result<HostSyncReport, ApplyError> {
        let mut report = HostSyncReport::ok();
        let root = super::config::user_dir();
        let user_tier = provider_tier() == ProviderTier::User;
        if ctx.manifest.skills.is_empty() {
            skills().clear()?;
        } else {
            skills().apply(ctx.manifest)?;
        }
        let servers = if user_tier {
            ctx.manifest.managed_mcp_servers.as_slice()
        } else {
            &[]
        };
        write_mcp_servers(ctx.loopback, servers)?;
        write_hook_plugin(ctx.loopback, ctx.manifest)?;
        content::write_agents_md(&root, &ctx.manifest.rules)?;
        let files = content::bundle_files(ctx.manifest, ctx.org_plugins_root, &mut report);
        let themes: Vec<String> = files
            .keys()
            .filter_map(|k| k.strip_prefix("themes/")?.strip_suffix(".json"))
            .map(str::to_owned)
            .collect();
        owned::sync(&root, &files)?;
        apply_cli_defaults(ctx.manifest)?;
        select_default_theme(ctx.manifest, &themes)?;
        Ok(report)
    }

    fn clear(&self, ctx: &HostSyncCtx<'_>) -> Result<(), ApplyError> {
        write_mcp_servers(ctx.loopback, &[])?;
        remove_owned_content()
    }
}
