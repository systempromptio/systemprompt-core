//! [`RunnerService`]: long-running work an extension runs on a `runner` node.
//!
//! A node with `server.role: runner` serves only its probes and runs no
//! scheduler, so work that must run continuously and scale apart from the
//! request path (a queue poller that spawns subprocesses, say) is declared
//! through `Extension::runner_services` instead of a cron job. The host starts
//! each service once per runner replica as a supervised background task and
//! cancels its token on shutdown; services on several replicas coordinate
//! through the database themselves (`FOR UPDATE SKIP LOCKED`). No other role
//! starts them.
//!
//! The service reaches the host's handles through the [`JobContext`] it is
//! given, exactly as a job does: the kernel's open seeds (`DbPool`,
//! `Arc<AppPaths>`, the file storage, …) and every admitted member's
//! provided values. Sealed seeds are withheld and the application context is
//! not in it.
//!
//! [`RunnerService::requires`] lists the types the service cannot run
//! without, checked against the runner's context with
//! [`StateRequirement::is_met_by`] before the service starts, so an unlinked
//! or disabled provider refuses the runner's boot instead of ending the
//! service on its first read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use async_trait::async_trait;
use systemprompt_provider_contracts::{JobContext, StateRequirement};
use tokio_util::sync::CancellationToken;

/// A service runs until its `shutdown` token is cancelled. Returning early ends
/// the service for the life of the process; it is not restarted.
///
/// Services are held as `Arc<dyn RunnerService>`, so the trait uses
/// `#[async_trait]`: native `async fn` in traits is not `dyn`-compatible.
#[async_trait]
pub trait RunnerService: Send + Sync + 'static {
    fn name(&self) -> &'static str;

    fn requires(&self) -> Vec<StateRequirement> {
        Vec::new()
    }

    async fn run(&self, ctx: JobContext, shutdown: CancellationToken);
}
