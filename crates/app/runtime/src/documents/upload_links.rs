//! Signed URLs for the resumable upload API, for a client that holds no
//! session cookie: an agent handed a session by `documents_upload`, or a
//! link that downloads a completed upload.
//!
//! Each URL carries a user JWT minted for the caller with the audience
//! [`UPLOAD_AUDIENCE`] and a `purpose` naming one upload: a write token
//! (`document_upload:<id>`) stores parts, reads progress, completes, aborts
//! and downloads that one upload; a read token
//! (`document_upload_download:<id>`) only downloads it. The upload routes
//! verify the token, check the purpose against the path and then act as
//! the token's subject, so the session's owner check still applies.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use systemprompt_identifiers::{AccessTokenId, DocumentUploadId, SessionId};
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, TokenAuthority, UserJwtRequest};
use systemprompt_models::modules::ApiPaths;

use super::download::{DOWNLOAD_TOKEN_QUERY, DownloadLinkError};

pub const UPLOAD_AUDIENCE: &str = "documents.upload";
pub const UPLOAD_LINK_TTL: Duration = Duration::hours(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UploadGrant {
    Write,
    Download,
}

#[must_use]
pub fn upload_purpose(grant: UploadGrant, upload: &DocumentUploadId) -> String {
    match grant {
        UploadGrant::Write => format!("document_upload:{upload}"),
        UploadGrant::Download => format!("document_upload_download:{upload}"),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UploadLinkTarget<'a> {
    pub grant: UploadGrant,
    pub base_url: &'a str,
    pub user: &'a AuthenticatedUser,
    pub session_id: &'a SessionId,
    pub upload: &'a DocumentUploadId,
}

/// The URLs one signed token opens. `parts_url` holds a literal `{n}` the
/// client replaces with the 1-based part number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedUploadLinks {
    pub status_url: String,
    pub parts_url: String,
    pub complete_url: String,
    pub content_url: String,
    pub expires_at: DateTime<Utc>,
}

pub fn signed_upload_links(
    authority: &dyn TokenAuthority,
    target: UploadLinkTarget<'_>,
) -> Result<SignedUploadLinks, DownloadLinkError> {
    let request = UserJwtRequest {
        user: target.user.clone(),
        session_id: target.session_id.clone(),
        jti: AccessTokenId::generate(),
        permissions: target.user.permissions().to_vec(),
        audience: vec![JwtAudience::Resource(UPLOAD_AUDIENCE.to_owned())],
        ttl: UPLOAD_LINK_TTL,
        client_id: None,
        purpose: Some(upload_purpose(target.grant, target.upload)),
        plugin_id: None,
    };
    let token = authority
        .mint_user_jwt(&request)
        .map_err(DownloadLinkError::from)?;
    let base = format!(
        "{}{}/{}",
        target.base_url.trim_end_matches('/'),
        ApiPaths::UPLOADS,
        target.upload
    );
    let query = format!("{DOWNLOAD_TOKEN_QUERY}={token}");
    Ok(SignedUploadLinks {
        status_url: format!("{base}?{query}"),
        parts_url: format!("{base}/parts/{{n}}?{query}"),
        complete_url: format!("{base}/complete?{query}"),
        content_url: format!("{base}/content?{query}"),
        expires_at: Utc::now() + UPLOAD_LINK_TTL,
    })
}
