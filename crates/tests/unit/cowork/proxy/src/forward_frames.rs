use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt};
use http_body_util::{BodyStream, StreamBody};
use hyper::HeaderMap;
use hyper::body::Frame;

#[tokio::test]
async fn data_frame_followed_by_trailers_yields_only_data() {
    let data = Frame::<Bytes>::data(Bytes::from_static(b"hello"));
    let mut trailers = HeaderMap::new();
    trailers.insert("x-checksum", "abc".parse().unwrap());
    let trailers = Frame::<Bytes>::trailers(trailers);

    let frames: Vec<Result<Frame<Bytes>, std::io::Error>> = vec![Ok(data), Ok(trailers)];
    let stream = futures_util::stream::iter(frames);
    let body = StreamBody::new(stream);

    let mut emitted: Vec<Bytes> = BodyStream::new(body)
        .try_filter_map(|frame: Frame<Bytes>| async move { Ok(frame.into_data().ok()) })
        .map_err(std::io::Error::other)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .map(|r| r.expect("frame ok"))
        .collect();

    assert_eq!(emitted.len(), 1, "trailers must not produce a data frame");
    assert_eq!(emitted.remove(0), Bytes::from_static(b"hello"));
}

#[tokio::test]
async fn empty_data_frames_are_preserved_but_trailers_are_dropped() {
    let d1 = Frame::<Bytes>::data(Bytes::from_static(b"a"));
    let empty = Frame::<Bytes>::data(Bytes::new());
    let mut trailers = HeaderMap::new();
    trailers.insert("x-final", "1".parse().unwrap());
    let trailers = Frame::<Bytes>::trailers(trailers);

    let frames: Vec<Result<Frame<Bytes>, std::io::Error>> =
        vec![Ok(d1), Ok(empty), Ok(trailers)];
    let body = StreamBody::new(futures_util::stream::iter(frames));

    let collected: Vec<Bytes> = BodyStream::new(body)
        .try_filter_map(|frame: Frame<Bytes>| async move { Ok(frame.into_data().ok()) })
        .map_err(std::io::Error::other)
        .map(|r| r.expect("ok"))
        .collect()
        .await;

    assert_eq!(collected.len(), 2);
    assert_eq!(collected[0], Bytes::from_static(b"a"));
    assert!(collected[1].is_empty(), "explicit empty data frame survives");
}
