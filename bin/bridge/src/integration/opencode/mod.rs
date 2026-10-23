//! `OpenCode` host integration: one configuration tree, two packagings.
//!
//! `OpenCode` ships as a terminal agent (`opencode-cli`) and an Electron
//! desktop app (`opencode`). `OpenCode` 2.x loads only the user's global
//! config (`~/.config/opencode/opencode.json`, on every platform), so the
//! bridge-owned `provider.systemprompt` block — `apiKey` included — the
//! `enabled_providers` allowlist and the default `model` are merged there,
//! beside the MCP connectors, by one writer and one sync emitter shared by
//! both packagings. That is placement, not enforcement: nothing stops a user
//! editing the file or connecting another provider, and sync rewrites it.
//! Governance is enforced at the gateway. The two
//! [`HostApp`](crate::integration::host_app::HostApp) implementations differ
//! only in how the packaging is detected (app bundle or installer folder versus
//! `opencode` on `PATH`), which processes count as it running, and whether it
//! can be launched.
//!
//! `OpenCode` lists the models the catalog advertises to `openai` clients
//! (`model_view::host_client`). It speaks the OpenAI-compatible wire, and the
//! gateway transcodes that wire to any provider's, so a Claude or Gemini model
//! reaches `OpenCode` whenever its catalog entry lists `openai` among its
//! `clients`; the catalog, not the provider's native wire, is the rule.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod hosts;
mod install;
mod managed_resources;
mod probe;

pub use hosts::{OPENCODE_APP_HOST, OPENCODE_CLI_HOST, OpenCodeAppHost, OpenCodeCliHost};
pub use managed_resources::OpenCodeSync;

use systemprompt_models::bridge::host::HostKind;

// Why: the shared config carries one provider `apiKey`, so one packaging's id
// is the token scope for both; `opencode` keeps every config written before
// the CLI became its own host valid without a re-mint.
pub(crate) const TOKEN_HOST: HostKind = HostKind::OpenCode;

crate::register_host_sync!(OpenCodeSync);

pub(crate) fn feedback_skill_root() -> std::path::PathBuf {
    config::skills_dir()
}

#[must_use]
pub fn retired_admin_tier() -> Option<std::path::PathBuf> {
    install::stale_admin_tier()
}
