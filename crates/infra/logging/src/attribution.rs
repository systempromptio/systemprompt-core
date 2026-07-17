//! Logging-side attribution cell.
//!
//! `tracing` macros fire from contexts where no `AppContext` handle is in
//! scope (gateway access logs, OTLP ingest, panic hooks). Threading a
//! resolved owner through every `info!()` call site is impractical, so the
//! platform parks the resolved [`SystemAdmin`] in a logging-private
//! `OnceLock` during runtime bootstrap. Only [`platform_attribution`] reads it.
//!
//! This is the *only* legitimate global owned by the logging crate. Other
//! subsystems (MCP registry, scheduler) thread their owner explicitly through
//! `AppContext` instead of consulting a process-wide cell.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::OnceLock;
use systemprompt_identifiers::UserId;
use systemprompt_models::services::SystemAdmin;
use thiserror::Error;

static PLATFORM_OWNER: OnceLock<SystemAdmin> = OnceLock::new();

/// On a repeat call the argument is dropped; the first-installed value is
/// returned.
pub fn install_log_attribution(admin: SystemAdmin) -> &'static SystemAdmin {
    PLATFORM_OWNER.get_or_init(|| admin)
}

pub fn platform_attribution() -> Result<&'static SystemAdmin, LogAttributionUnset> {
    PLATFORM_OWNER.get().ok_or(LogAttributionUnset)
}

pub(crate) fn platform_owner_id() -> Result<&'static UserId, LogAttributionUnset> {
    platform_attribution().map(SystemAdmin::id)
}

#[derive(Debug, Clone, Copy, Error)]
#[error("log attribution not installed: AppContext bootstrap must run before platform log events")]
pub struct LogAttributionUnset;
