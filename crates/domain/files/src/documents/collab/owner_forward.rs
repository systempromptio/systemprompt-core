//! Forwarding one collab update to the replica that owns its document.
//!
//! The raw request travels unchanged — its body, content type, query and
//! the caller's `Authorization` and `Cookie` headers — to the same route on the
//! owner, marked with [`FORWARDED_HEADER`] so the owner applies it rather than
//! forwarding again. The owner authenticates and authorises the caller
//! itself. Any transport failure is returned to the caller, which then
//! applies the update locally.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::CollabService;

pub const FORWARDED_HEADER: &str = "x-systemprompt-collab-forwarded";

#[derive(Debug, Clone)]
pub struct ForwardRequest<'a> {
    pub address: &'a str,
    pub path_and_query: &'a str,
    pub authorization: Option<&'a str>,
    pub cookie: Option<&'a str>,
    pub content_type: Option<&'a str>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forwarded {
    pub status: u16,
    pub content_type: Option<String>,
    pub retry_after: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("no collab owner forwarding is configured on this replica")]
    Disabled,
    #[error("collab update not forwarded to its owner: {0}")]
    Transport(#[from] reqwest::Error),
}

impl CollabService {
    pub async fn forward_update(
        &self,
        request: ForwardRequest<'_>,
    ) -> Result<Forwarded, ForwardError> {
        let owner = self.owner.as_ref().ok_or(ForwardError::Disabled)?;
        let url = format!(
            "{}{}",
            request.address.trim_end_matches('/'),
            request.path_and_query
        );
        let mut outbound = owner
            .client
            .post(url)
            .header(FORWARDED_HEADER, owner.id.to_string())
            .body(request.body);
        if let Some(authorization) = request.authorization {
            outbound = outbound.header(reqwest::header::AUTHORIZATION, authorization);
        }
        if let Some(cookie) = request.cookie {
            outbound = outbound.header(reqwest::header::COOKIE, cookie);
        }
        if let Some(content_type) = request.content_type {
            outbound = outbound.header(reqwest::header::CONTENT_TYPE, content_type);
        }
        let response = outbound.send().await?;
        let header = |name: reqwest::header::HeaderName| {
            response
                .headers()
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };
        let content_type = header(reqwest::header::CONTENT_TYPE);
        let retry_after = header(reqwest::header::RETRY_AFTER);
        let status = response.status().as_u16();
        let body = response.bytes().await?.to_vec();
        Ok(Forwarded {
            status,
            content_type,
            retry_after,
            body,
        })
    }
}
