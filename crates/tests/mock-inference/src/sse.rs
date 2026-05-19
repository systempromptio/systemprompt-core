//! Shared SSE response construction for both protocol handlers.

use std::convert::Infallible;
use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use serde_json::Value;

use crate::Mode;

/// Serialises a JSON value as a single SSE `data:` frame.
pub fn frame(value: &Value) -> String {
    format!("data: {value}\n\n")
}

/// Builds a streaming `text/event-stream` response from pre-rendered frames.
/// In `SlowLoris` mode each frame is trickled out after a delay; otherwise
/// frames are yielded back-to-back.
pub fn response(frames: Vec<String>, mode: Mode) -> Response {
    let trickle = matches!(mode, Mode::SlowLoris);
    let stream = async_stream::stream! {
        for frame in frames {
            if trickle {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            yield Ok::<_, Infallible>(frame.into_bytes());
        }
    };
    let mut resp = Response::new(Body::from_stream(stream));
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream"),
    );
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    resp
}
