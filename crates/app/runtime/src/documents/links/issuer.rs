//! The MCP-server side: asking the API for a signed link.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use async_trait::async_trait;
use systemprompt_client::{GuardedClientConfig, guarded_client};
use systemprompt_identifiers::{JwtToken, McpServerId, headers};
use systemprompt_models::modules::ApiPaths;

use super::{IssuedLink, LinkRequest, LinkTarget};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum LinkIssueError {
    #[error("the call carries no hop token, so the API cannot be asked for a signed link")]
    MissingHop,
    #[error("the link client could not be built")]
    Client(#[source] reqwest::Error),
    #[error("the API could not be reached for a signed link")]
    Transport(#[source] reqwest::Error),
    #[error("the API refused the link request with status {status}: {body}")]
    Refused { status: u16, body: String },
    #[error("the API's link response could not be read")]
    Decode(#[source] reqwest::Error),
    #[error("the API answered a different kind of link than was requested")]
    Mismatched,
}

/// Obtains signed document links on behalf of the call being served.
///
/// `#[async_trait]` keeps it `dyn`-compatible: the tools hold an
/// `Arc<dyn LinkIssuer>`.
#[async_trait]
pub trait LinkIssuer: Send + Sync + std::fmt::Debug {
    async fn issue(
        &self,
        hop: Option<&JwtToken>,
        target: LinkTarget,
    ) -> Result<IssuedLink, LinkIssueError>;
}

/// [`LinkIssuer`] over `POST /api/v1/internal/links` on the API.
#[derive(Debug, Clone)]
pub struct ApiLinkIssuer {
    client: reqwest::Client,
    endpoint: String,
    server: McpServerId,
}

impl ApiLinkIssuer {
    pub fn new(api_internal_url: &str, server: McpServerId) -> Result<Self, LinkIssueError> {
        let client =
            guarded_client(&GuardedClientConfig::for_configured().with_timeout(REQUEST_TIMEOUT))
                .map_err(LinkIssueError::Client)?;
        Ok(Self {
            client,
            endpoint: format!(
                "{}{}",
                api_internal_url.trim_end_matches('/'),
                ApiPaths::INTERNAL_LINKS
            ),
            server,
        })
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait]
impl LinkIssuer for ApiLinkIssuer {
    async fn issue(
        &self,
        hop: Option<&JwtToken>,
        target: LinkTarget,
    ) -> Result<IssuedLink, LinkIssueError> {
        let hop = hop.ok_or(LinkIssueError::MissingHop)?;
        let response = self
            .client
            .post(&self.endpoint)
            .header(headers::MCP_HOP, hop.as_str())
            .json(&LinkRequest {
                server: self.server.clone(),
                target,
            })
            .send()
            .await
            .map_err(LinkIssueError::Transport)?;
        let status = response.status();
        if !status.is_success() {
            let body = response
                .text()
                .await
                .unwrap_or_else(|error| format!("<unreadable body: {error}>"));
            return Err(LinkIssueError::Refused {
                status: status.as_u16(),
                body,
            });
        }
        response.json().await.map_err(LinkIssueError::Decode)
    }
}
