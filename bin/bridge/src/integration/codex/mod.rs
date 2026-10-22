//! Codex host integration: one configuration tree, two packagings.
//!
//! Codex ships as a desktop app (`codex`) and a CLI (`codex-cli`). Both read
//! `CODEX_HOME` (`~/.codex`) and the managed tier above it (`/etc/codex`, the
//! Windows `managed_config.toml`, the macOS `com.openai.codex` MDM payload),
//! so there is one config writer, one sync emitter and one loopback
//! credential, written once and read by whichever packagings are installed.
//! The two [`HostApp`] implementations differ only in how the packaging is
//! detected (app bundle or installer folder versus `codex` on `PATH`), which
//! processes count as it running, and whether it can be launched.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod config;
mod hosts;
mod install;
mod managed_resources;
mod probe;

pub use hosts::{CODEX_APP_HOST, CODEX_CLI_HOST, CodexAppHost, CodexCliHost};
pub use managed_resources::CodexSync;

use systemprompt_models::bridge::host::HostKind;

// Why: the shared config carries one credential helper invocation, so one
// packaging's id is the token scope for both; the CLI's id keeps every config
// written before the app became its own host valid without a re-mint.
pub(crate) const TOKEN_HOST: HostKind = HostKind::CodexCli;

crate::register_host_sync!(CodexSync);

pub(crate) use managed_resources::feedback_skill_roots;
