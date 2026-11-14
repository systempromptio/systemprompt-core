use anyhow::Result;
use axum::body::Body;
use axum::http::{Response, StatusCode};
use http_body_util::BodyExt;

pub async fn body_to_string(resp: Response<Body>) -> Result<(StatusCode, String)> {
    let status = resp.status();
    let body = resp.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}
