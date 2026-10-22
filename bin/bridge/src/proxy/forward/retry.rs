//! One resend with a re-minted gateway token when the gateway rejects a
//! cached one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use hyper::StatusCode;
use reqwest::header::HeaderMap;

use crate::proxy::token_cache::TokenCache;

use super::replay::{UpstreamRequest, replay_policy, send_with_replay};
use super::{ForwardResult, REFRESH_THRESHOLD_SECS, headers};

pub(super) struct UpstreamSend<'a> {
    pub client: &'a reqwest::Client,
    pub method: &'a reqwest::Method,
    pub url: &'a str,
    pub body: &'a Bytes,
    pub request_path: &'a str,
    pub gateway_signed: bool,
}

impl UpstreamSend<'_> {
    fn request<'h>(&'h self, headers: &'h HeaderMap) -> UpstreamRequest<'h> {
        UpstreamRequest {
            client: self.client,
            method: self.method,
            url: self.url,
            headers,
            body: self.body,
            policy: replay_policy(self.request_path, self.body),
        }
    }
}

// Why: OpenCode answers a 401 by starting MCP OAuth against the proxy; a
// gateway redeploy rejects a cached JWT that still looks valid, so re-mint
// once and resend instead of passing it on.
pub(super) async fn send_upstream(
    send: UpstreamSend<'_>,
    token_cache: &TokenCache,
    mut upstream_headers: HeaderMap,
) -> ForwardResult<reqwest::Response> {
    let response = send_with_replay(send.request(&upstream_headers)).await?;
    if !(send.gateway_signed
        && response.status() == StatusCode::UNAUTHORIZED
        && token_cache.reject_upstream(send.request_path).await)
    {
        return Ok(response);
    }
    let fresh = token_cache.current(REFRESH_THRESHOLD_SECS).await?;
    upstream_headers.insert(
        reqwest::header::AUTHORIZATION,
        headers::bearer_value(fresh.token.expose())?,
    );
    let response = send_with_replay(send.request(&upstream_headers)).await?;
    if response.status() == StatusCode::UNAUTHORIZED {
        token_cache.reject_upstream(send.request_path).await;
    }
    Ok(response)
}
