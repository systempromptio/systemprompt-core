//! A served proxy connection that ends because the upstream stream went idle
//! past the read limit is told apart from a genuine failure, so a held-open
//! SSE response timing out is not logged as a proxy fault.

use std::convert::Infallible;
use std::time::Duration;

use bytes::Bytes;
use futures_util::TryStreamExt;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use systemprompt_bridge::proxy::{ConnectionEnd, classify_connection_end};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Body = BoxBody<Bytes, std::io::Error>;

async fn stalling_upstream() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0u8; 1024];
        let _read = socket.read(&mut request).await.unwrap();
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\
                  Transfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n",
            )
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(30)).await;
        drop(socket);
    });
    format!("http://{address}/stream")
}

async fn serve_one(body: Body) -> hyper::Error {
    let (client, server) = tokio::io::duplex(64 * 1024);
    let slot = std::sync::Mutex::new(Some(body));
    let svc = service_fn(move |_req: Request<Incoming>| {
        let body = slot.lock().unwrap().take().expect("one request");
        async move { Ok::<_, Infallible>(Response::new(body)) }
    });
    let serving = tokio::spawn(async move {
        http1::Builder::new()
            .serve_connection(TokioIo::new(server), svc)
            .await
    });
    let (mut read, mut write) = tokio::io::split(client);
    write
        .write_all(b"GET /stream HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
        .await
        .unwrap();
    let mut sink = Vec::new();
    let _read = read.read_to_end(&mut sink).await;
    serving
        .await
        .unwrap()
        .expect_err("a body that fails mid-stream ends the connection with an error")
}

#[tokio::test]
async fn an_upstream_stream_idle_past_the_read_limit_is_classified_idle() {
    let url = stalling_upstream().await;
    let upstream = reqwest::Client::builder()
        .read_timeout(Duration::from_millis(200))
        .build()
        .unwrap()
        .get(url)
        .send()
        .await
        .unwrap();
    let body = StreamBody::new(
        upstream
            .bytes_stream()
            .map_ok(Frame::data)
            .map_err(std::io::Error::other),
    )
    .boxed();

    let error = serve_one(body).await;

    assert_eq!(classify_connection_end(&error), ConnectionEnd::UpstreamIdle);
}

#[tokio::test]
async fn a_body_failure_that_is_not_an_upstream_timeout_stays_a_failure() {
    let failing = futures_util::stream::iter(vec![
        Ok(Frame::data(Bytes::from_static(b"hello"))),
        Err(std::io::Error::other("tap decoder broke")),
    ]);
    let error = serve_one(StreamBody::new(failing).boxed()).await;

    assert_eq!(classify_connection_end(&error), ConnectionEnd::Failed);
}
