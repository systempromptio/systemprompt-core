//! Signed document links for MCP servers, minted only by the API.
//!
//! A managed MCP server holds no signing key, so the documents and issues
//! tools ask the API for a signed download or upload link through a
//! `LinkIssuer`. `ApiLinkIssuer` posts the request to
//! `POST /api/v1/internal/links`, presenting the hop token the proxy minted
//! for the call being served; the API verifies that token for the named
//! server, takes the subject, the session and the acting agent from its claims
//! alone, re-checks that the subject may open the target (the upload session
//! is theirs and still open; the document revision is visible to them, acting
//! as that agent), and only then signs the link (`mint_link`). A request
//! without a hop token never leaves the server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod issuer;
mod mint;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, DocumentUploadId, McpServerId};

pub use issuer::{ApiLinkIssuer, LinkIssueError, LinkIssuer, LinkRefusal};
pub use mint::{LinkMintError, LinkMinter, mint_link, mint_verified, verify_link_hop};

use super::download::Rendition;
use super::upload_links::SignedUploadLinks;

/// The link an MCP server asks the API to sign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LinkTarget {
    Upload {
        upload_id: DocumentUploadId,
    },
    Download {
        document_id: DocumentId,
        revision_id: DocumentRevisionId,
        rendition: Rendition,
    },
}

/// The body of `POST /api/v1/internal/links`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRequest {
    pub server: McpServerId,
    pub target: LinkTarget,
}

/// The link the API signed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IssuedLink {
    Upload(SignedUploadLinks),
    Download {
        url: String,
        expires_at: DateTime<Utc>,
    },
}

pub async fn issue_upload_links(
    issuer: &dyn LinkIssuer,
    hop: Option<&systemprompt_identifiers::JwtToken>,
    upload_id: &DocumentUploadId,
) -> Result<SignedUploadLinks, LinkIssueError> {
    match issuer
        .issue(
            hop,
            LinkTarget::Upload {
                upload_id: upload_id.clone(),
            },
        )
        .await?
    {
        IssuedLink::Upload(links) => Ok(links),
        IssuedLink::Download { .. } => Err(LinkIssueError::Mismatched),
    }
}

pub async fn issue_download_link(
    issuer: &dyn LinkIssuer,
    hop: Option<&systemprompt_identifiers::JwtToken>,
    target: LinkTarget,
) -> Result<(String, DateTime<Utc>), LinkIssueError> {
    match issuer.issue(hop, target).await? {
        IssuedLink::Download { url, expires_at } => Ok((url, expires_at)),
        IssuedLink::Upload(_) => Err(LinkIssueError::Mismatched),
    }
}
