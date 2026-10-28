//! Signed upload tokens for the resumable upload API.
//!
//! They serve a client that holds no session cookie: an agent handed a
//! session by `documents_upload`, or a link that downloads a completed
//! upload.
//!
//! A token is a user JWT minted for the caller with **no permissions**, the
//! audience `UPLOAD_AUDIENCE` (which the API's session middleware does not
//! accept, so it is no general credential), a fifteen-minute lifetime and a
//! `purpose` naming one upload. A write token (`document_upload:<id>`)
//! opens only that upload's `PUT /{id}/parts/{n}`, `POST /{id}/complete`
//! and `GET /{id}` progress; a download token
//! (`document_upload_download:<id>`) opens only its `GET|HEAD
//! /{id}/content`. The token travels in the `UPLOAD_TOKEN_HEADER` header,
//! so it stays out of URLs and access logs; the `access_token` query is
//! accepted only for a client that cannot set headers. The upload routes
//! act as the token's subject, so the session's owner check still applies.
//! A client whose token ran out asks `documents_upload` again with the
//! `upload_id` for a fresh one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use systemprompt_identifiers::{AccessTokenId, DocumentUploadId, SessionId};
use systemprompt_models::auth::{AuthenticatedUser, JwtAudience, TokenAuthority, UserJwtRequest};
use systemprompt_models::modules::ApiPaths;

use super::download::DownloadLinkError;

pub const UPLOAD_AUDIENCE: &str = "documents.upload";
pub const UPLOAD_LINK_TTL: Duration = Duration::minutes(15);
pub const UPLOAD_TOKEN_HEADER: &str = "x-upload-token";

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

/// One signed token and the token-free URLs it opens, sent with the token
/// in [`UPLOAD_TOKEN_HEADER`]. `parts_url` holds a literal `{n}` the client
/// replaces with the 1-based part number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedUploadLinks {
    pub token: String,
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
        permissions: Vec::new(),
        audience: vec![JwtAudience::Resource(UPLOAD_AUDIENCE.to_owned())],
        ttl: UPLOAD_LINK_TTL,
        client_id: None,
        purpose: Some(upload_purpose(target.grant, target.upload)),
        plugin_id: None,
        interaction: None,
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
    Ok(SignedUploadLinks {
        token,
        status_url: base.clone(),
        parts_url: format!("{base}/parts/{{n}}"),
        complete_url: format!("{base}/complete"),
        content_url: format!("{base}/content"),
        expires_at: Utc::now() + UPLOAD_LINK_TTL,
    })
}
