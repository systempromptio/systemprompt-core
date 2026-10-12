//! Supervises the wall clock and the cancellation token, and reaps the child.
//!
//! The child's exit, the caller's cancel and the run's wall clock race; a
//! cancel or a timeout ends the whole process group gracefully (`SIGTERM`,
//! five seconds, `SIGKILL`). Whatever ends the run, the group is killed once
//! the leader is gone so a background grandchild cannot hold the output
//! pipes open.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::spawn::OwnedChild;
use crate::host::ExitInfo;

pub(super) async fn supervise(
    owned: &mut OwnedChild,
    wall_clock: Duration,
    cancel: &CancellationToken,
    started: Instant,
) -> ExitInfo {
    let mut timed_out = false;
    let mut cancelled = false;
    let code = tokio::select! {
        status = owned.child.wait() => match status {
            Ok(status) => status.code(),
            Err(error) => {
                tracing::warn!(%error, "headless child wait failed");
                None
            },
        },
        () = cancel.cancelled() => {
            cancelled = true;
            owned.terminate_group_gracefully().await
        },
        () = tokio::time::sleep(wall_clock) => {
            timed_out = true;
            owned.terminate_group_gracefully().await
        },
    };
    owned.guard.kill_group();
    ExitInfo {
        code,
        timed_out,
        cancelled,
        duration_ms: i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
    }
}
