//! Tests for `guarded_stream`.

use std::io::{Error, ErrorKind};
use std::time::Duration;

use futures_util::stream::{self, StreamExt};
use systemprompt_database::resilience::bulkhead::Bulkhead;
use systemprompt_database::resilience::stream::guarded_stream;

fn idle_error(_: Duration) -> Error {
    Error::new(ErrorKind::TimedOut, "stream idle timeout")
}

#[tokio::test]
async fn passes_items_through_and_releases_the_permit() {
    let bulkhead = Bulkhead::new("dep", 1);
    let permit = bulkhead.try_acquire().expect("permit");

    let inner = stream::iter(vec![Ok::<u32, Error>(1), Ok(2), Ok(3)]);
    let wrapped = guarded_stream(inner, Duration::from_secs(5), permit, idle_error);
    let items: Vec<_> = wrapped.collect().await;

    assert_eq!(items.len(), 3);
    assert!(items.iter().all(Result::is_ok));
    // The permit is released once the stream ends.
    assert!(bulkhead.try_acquire().is_ok());
}

#[tokio::test]
async fn aborts_when_a_chunk_stalls_past_the_idle_timeout() {
    let bulkhead = Bulkhead::new("dep", 1);
    let permit = bulkhead.try_acquire().expect("permit");

    let inner = stream::once(async { Ok::<u32, Error>(1) }).chain(stream::once(async {
        tokio::time::sleep(Duration::from_secs(3600)).await;
        Ok(2)
    }));
    let wrapped = guarded_stream(inner, Duration::from_millis(20), permit, idle_error);
    let items: Vec<_> = wrapped.collect().await;

    assert_eq!(items.len(), 2);
    assert!(items[0].is_ok());
    assert!(items[1].is_err());
    // The permit is released even on an idle-timeout abort.
    assert!(bulkhead.try_acquire().is_ok());
}
