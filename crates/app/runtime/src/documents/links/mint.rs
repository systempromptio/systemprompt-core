//! The API side: verifying a link request and signing the link.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_files::documents::agents::act_as;
use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_files::uploads::{UploadError, UploadService, UploadStatus};
use systemprompt_identifiers::{McpServerId, TraceId};
use systemprompt_loader::ServicesState;
use systemprompt_models::auth::{self, McpHopClaims, TokenAuthority, TokenError};

use super::{IssuedLink, LinkRequest, LinkTarget};
use crate::documents::actor::bearer_actor;
use crate::documents::download::{DownloadLinkError, DownloadTarget, signed_download_url};
use crate::documents::upload_links::{UploadGrant, UploadLinkTarget, signed_upload_links};
use crate::mcp_host::{DOCUMENTS_TOOLSET, ISSUES_TOOLSET};

#[derive(Debug, thiserror::Error)]
pub enum LinkMintError {
    #[error("the hop token was refused")]
    Hop(#[source] TokenError),
    #[error("the MCP server '{server}' hosts no toolset that may request this link")]
    NotHosted { server: McpServerId },
    #[error("the hop token names no authenticated user")]
    Anonymous,
    #[error("the upload is not the caller's")]
    Upload(#[source] UploadError),
    #[error("the upload is {status}, not open")]
    UploadClosed { status: UploadStatus },
    #[error("the document revision is not visible to the caller")]
    Document(#[source] DocumentsError),
    #[error("the link could not be signed")]
    Signing(#[source] DownloadLinkError),
}

/// What [`mint_link`] signs and checks with.
#[derive(Clone)]
pub struct LinkMinter {
    pub tokens: Arc<dyn TokenAuthority>,
    pub documents: Arc<DocumentService>,
    pub uploads: Arc<UploadService>,
    pub services: ServicesState,
    pub api_external_url: String,
}

impl std::fmt::Debug for LinkMinter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkMinter")
            .field("api_external_url", &self.api_external_url)
            .finish_non_exhaustive()
    }
}

pub async fn mint_link(
    minter: &LinkMinter,
    hop: &str,
    request: &LinkRequest,
) -> Result<IssuedLink, LinkMintError> {
    let claims = minter
        .tokens
        .verify_mcp_hop(hop, &request.server)
        .map_err(LinkMintError::Hop)?;
    ensure_hosted(&minter.services, request)?;
    match &request.target {
        LinkTarget::Upload { upload_id } => {
            let user = claims
                .identity
                .user
                .as_ref()
                .ok_or(LinkMintError::Anonymous)?;
            let progress = minter
                .uploads
                .progress(&user.id, upload_id)
                .await
                .map_err(LinkMintError::Upload)?;
            if progress.session.status != UploadStatus::Open {
                return Err(LinkMintError::UploadClosed {
                    status: progress.session.status,
                });
            }
            let links = signed_upload_links(
                minter.tokens.as_ref(),
                UploadLinkTarget {
                    grant: UploadGrant::Write,
                    base_url: &minter.api_external_url,
                    user,
                    session_id: &claims.identity.session_id,
                    upload: upload_id,
                },
            )
            .map_err(LinkMintError::Signing)?;
            Ok(IssuedLink::Upload(links))
        },
        LinkTarget::Download {
            document_id,
            revision_id,
            rendition,
        } => {
            let identity = &claims.identity;
            let user = identity.user.as_ref().ok_or(LinkMintError::Anonymous)?;
            let actor = bearer_actor(
                identity.purpose.as_deref(),
                identity.client_id.as_ref(),
                identity.interaction,
            );
            act_as(
                actor,
                minter
                    .documents
                    .revision(user, document_id, revision_id, &TraceId::generate()),
            )
            .await
            .map_err(LinkMintError::Document)?;
            let agent = agent_of(&claims);
            let (url, expires_at) = signed_download_url(
                minter.tokens.as_ref(),
                DownloadTarget {
                    rendition: *rendition,
                    base_url: &minter.api_external_url,
                    user,
                    session_id: &identity.session_id,
                    document: document_id,
                    revision: revision_id,
                    agent: &agent,
                },
            )
            .map_err(LinkMintError::Signing)?;
            Ok(IssuedLink::Download { url, expires_at })
        },
    }
}

fn ensure_hosted(services: &ServicesState, request: &LinkRequest) -> Result<(), LinkMintError> {
    let permitted: &[&str] = match request.target {
        LinkTarget::Upload { .. } => &[DOCUMENTS_TOOLSET, ISSUES_TOOLSET],
        LinkTarget::Download { .. } => &[DOCUMENTS_TOOLSET],
    };
    let hosted = services
        .config()
        .mcp_servers
        .get(request.server.as_str())
        .is_some_and(|deployment| {
            deployment
                .enabled_toolsets()
                .any(|toolset| permitted.contains(&toolset.id.as_str()))
        });
    if hosted {
        Ok(())
    } else {
        Err(LinkMintError::NotHosted {
            server: request.server.clone(),
        })
    }
}

fn agent_of(claims: &McpHopClaims) -> systemprompt_identifiers::AgentName {
    auth::agent_name(
        claims.identity.purpose.as_deref(),
        claims.identity.client_id.as_ref(),
    )
}
