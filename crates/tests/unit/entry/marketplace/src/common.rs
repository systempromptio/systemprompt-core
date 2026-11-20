//! Shared helpers for the marketplace route tests: the marketplace route set
//! materialized over a fixture context's resolved kernel state, as the API
//! mount materializes it.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use http_body_util::BodyExt;
use systemprompt_marketplace_entry::http::public_routes;
use systemprompt_runtime::AppContext;

pub fn marketplace_router(ctx: &AppContext) -> Router {
    public_routes()
        .materialize(&ctx.kernel().resolved())
        .expect("the marketplace state resolves from the fixture kernel")
        .into_iter()
        .fold(Router::new(), Router::merge)
}

pub async fn body_to_string(resp: Response<Body>) -> Result<(http::StatusCode, String)> {
    let status = resp.status();
    let body = resp.into_body().collect().await?.to_bytes();
    Ok((status, String::from_utf8_lossy(&body).into_owned()))
}

pub fn empty_get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request build")
}
