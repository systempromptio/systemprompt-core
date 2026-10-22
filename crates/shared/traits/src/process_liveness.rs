//! [`ProcessLiveness`] — whether a recorded pid is a live child this
//! installation owns.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_identifiers::ServiceName;

/// Identity check behind every stale-row reaper.
///
/// A pid is live only when the process exists *and* carries this
/// installation's spawn marker for `service`, so a recycled pid never keeps a
/// dead service's row alive. `#[async_trait]` because the repository takes it
/// as `&dyn ProcessLiveness`.
#[async_trait]
pub trait ProcessLiveness: Send + Sync {
    async fn owns(&self, pid: u32, service: &ServiceName) -> bool;
}
