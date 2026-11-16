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
    #[error("the API refused the link request: {}", refusal.code())]
    Refused { status: u16, refusal: LinkRefusal },
    #[error("the API's link response could not be read")]
    Decode(#[source] reqwest::Error),
    #[error("the API answered a different kind of link than was requested")]
    Mismatched,
}

/// Why the API refused a link request, read from its status alone; the
/// API's body never reaches the tool result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRefusal {
    HopRefused,
    Forbidden,
    NotOpen,
    Failed,
    Unavailable,
}

impl LinkRefusal {
    #[must_use]
    pub const fn from_status(status: u16) -> Self {
        match status {
            401 => Self::HopRefused,
            403 => Self::Forbidden,
            409 => Self::NotOpen,
            500 => Self::Failed,
            _ => Self::Unavailable,
        }
    }

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::HopRefused => "hop_refused",
            Self::Forbidden => "forbidden",
            Self::NotOpen => "not_open",
            Self::Failed => "link_failed",
            Self::Unavailable => "link_endpoint_unavailable",
        }
    }
}

impl LinkIssueError {
    #[must_use]
    pub fn tool_message(&self, links: &str) -> String {
        match self {
            Self::MissingHop => {
                format!("the {links} cannot be issued for a call without a hop token")
            },
            Self::Client(_) | Self::Transport(_) => {
                format!("the link endpoint was unreachable, so the {links} could not be issued")
            },
            Self::Refused {
                status,
                refusal: LinkRefusal::Unavailable,
            } => format!("the link endpoint returned {status}, so the {links} could not be issued"),
            Self::Refused {
                refusal: LinkRefusal::Failed,
                ..
            } => format!("the API could not sign the {links}"),
            Self::Refused { refusal, .. } => {
                format!("the link endpoint refused the {links}: {}", refusal.code())
            },
            Self::Decode(_) | Self::Mismatched => {
                format!("the link endpoint's answer for the {links} could not be read")
            },
        }
    }
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
            return Err(LinkIssueError::Refused {
                status: status.as_u16(),
                refusal: LinkRefusal::from_status(status.as_u16()),
            });
        }
        response.json().await.map_err(LinkIssueError::Decode)
    }
}
