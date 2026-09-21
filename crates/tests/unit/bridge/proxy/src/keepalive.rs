use std::io;
use std::time::Duration;

use bytes::Bytes;
use futures_util::{FutureExt, StreamExt, stream};
use hyper::body::Frame;
use systemprompt_bridge::proxy::keepalive::SseKeepalive;

fn text(frame: Frame<Bytes>) -> String {
    String::from_utf8(frame.into_data().expect("data frame").to_vec()).expect("utf-8 frame")
}

#[tokio::test(start_paused = true)]
async fn an_idle_sse_stream_emits_a_keepalive_only_after_its_interval() {
    let interval = Duration::from_secs(15);
    let mut stream = SseKeepalive::new(stream::pending::<io::Result<Frame<Bytes>>>(), interval);

    assert!(
        stream.next().now_or_never().is_none(),
        "no frame before time advances"
    );
    tokio::time::advance(interval - Duration::from_millis(1)).await;
    assert!(stream.next().now_or_never().is_none(), "no early keepalive");
    tokio::time::advance(Duration::from_millis(1)).await;

    let due = stream
        .next()
        .now_or_never()
        .expect("keepalive is due without advancing time")
        .expect("keepalive frame")
        .expect("successful keepalive");
    assert_eq!(text(due), ": keepalive\n\n");
}

#[tokio::test(start_paused = true)]
async fn an_upstream_frame_resets_the_keepalive_deadline() {
    let interval = Duration::from_secs(15);
    let source = stream::iter(vec![Ok(Frame::data(Bytes::from_static(
        b"data: upstream\n\n",
    )))])
    .chain(stream::pending());
    let mut stream = SseKeepalive::new(source, interval);

    tokio::time::advance(Duration::from_secs(10)).await;
    assert_eq!(
        text(stream.next().await.expect("upstream frame").expect("frame")),
        "data: upstream\n\n"
    );
    tokio::time::advance(Duration::from_secs(5)).await;
    assert!(
        stream.next().now_or_never().is_none(),
        "the old deadline was reset"
    );
    tokio::time::advance(Duration::from_secs(10)).await;

    let due = stream
        .next()
        .now_or_never()
        .expect("reset deadline is due without advancing time")
        .expect("keepalive frame")
        .expect("successful keepalive");
    assert_eq!(text(due), ": keepalive\n\n");
}

#[tokio::test(start_paused = true)]
async fn an_ended_sse_stream_never_emits_a_keepalive() {
    let mut stream = SseKeepalive::new(
        stream::empty::<io::Result<Frame<Bytes>>>(),
        Duration::from_secs(15),
    );

    assert!(
        stream.next().await.is_none(),
        "the upstream end is preserved"
    );
    tokio::time::advance(Duration::from_secs(60)).await;
    assert!(
        stream
            .next()
            .now_or_never()
            .is_some_and(|next| next.is_none())
    );
}
