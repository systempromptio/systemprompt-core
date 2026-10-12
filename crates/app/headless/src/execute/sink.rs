//! [`EventSink`]: where a running host's decoded events go.
//!
//! The executor hands batches to the sink in arrival order; the repository
//! sink persists them in `headless_run_events`, a test sink collects them.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::HostSessionId;

use crate::error::HeadlessResult;
use crate::event::RunEvent;

/// Receives a run's events.
///
/// Held as `&dyn EventSink` by the executor so the
/// persistence target is chosen by the caller; `#[async_trait]` keeps the
/// trait `dyn`-compatible, which native `async fn` in traits is not.
#[async_trait]
pub trait EventSink: Send + Sync {
    async fn emit(&self, batch: Vec<RunEvent>) -> HeadlessResult<()>;

    async fn session_observed(&self, session_id: &HostSessionId) -> HeadlessResult<()>;
}
