//! Idle-timeout wrapper for streaming responses.

use std::sync::Arc;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use tokio::sync::OwnedSemaphorePermit;

/// Wrap `inner` so each chunk must arrive within `idle_timeout`.
///
/// `permit` (a bulkhead permit from
/// [`super::guard::ResilienceGuard::acquire_permit`]) is held until the stream
/// ends, so a streaming response counts against the dependency's concurrency
/// limit for its whole lifetime. If a chunk stalls past `idle_timeout` the
/// stream yields one final error built by `on_idle_timeout`, then terminates.
pub fn guarded_stream<S, T, E>(
    inner: S,
    idle_timeout: Duration,
    permit: OwnedSemaphorePermit,
    on_idle_timeout: impl Fn(Duration) -> E,
) -> impl Stream<Item = Result<T, E>>
where
    S: Stream<Item = Result<T, E>>,
{
    let on_idle_timeout = Arc::new(on_idle_timeout);
    let init = Some((Box::pin(inner), permit));
    futures_util::stream::unfold(init, move |state| {
        let on_idle_timeout = Arc::clone(&on_idle_timeout);
        async move {
            let (mut inner, permit) = state?;
            match tokio::time::timeout(idle_timeout, inner.next()).await {
                Ok(Some(item)) => Some((item, Some((inner, permit)))),
                Ok(None) => {
                    drop(permit);
                    None
                },
                Err(_) => {
                    drop(permit);
                    Some((Err(on_idle_timeout(idle_timeout)), None))
                },
            }
        }
    })
}
