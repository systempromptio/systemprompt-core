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

use std::sync::OnceLock;
use systemprompt_identifiers::UserId;
use systemprompt_models::services::SystemAdmin;
use thiserror::Error;

static PLATFORM_OWNER: OnceLock<SystemAdmin> = OnceLock::new();

/// Park the resolved system-admin in the logging-attribution cell.
///
/// Called once during `AppContext` bootstrap, immediately after the admin row
/// is resolved against the `users` table. Subsequent calls in the same process
/// observe the installed value and return it; the input is dropped.
pub fn install_log_attribution(admin: SystemAdmin) -> &'static SystemAdmin {
    PLATFORM_OWNER.get_or_init(|| admin)
}

// Errors with LogAttributionUnset before bootstrap has run. Reserved for
// crate::models::LogActor::platform.
pub fn platform_attribution() -> Result<&'static SystemAdmin, LogAttributionUnset> {
    PLATFORM_OWNER.get().ok_or(LogAttributionUnset)
}

pub(crate) fn platform_owner_id() -> Result<&'static UserId, LogAttributionUnset> {
    platform_attribution().map(SystemAdmin::id)
}

#[derive(Debug, Clone, Copy, Error)]
#[error("log attribution not installed: AppContext bootstrap must run before platform log events")]
pub struct LogAttributionUnset;
