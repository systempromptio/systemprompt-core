//! Short-lived download URLs for `documents_export`, verified by the HTTP
//! content route.
//!
//! The URL carries a user JWT minted by the deployment's
//! [`TokenAuthority`] for the caller, with the audience
//! `DOWNLOAD_AUDIENCE`, a ten-minute lifetime and a `purpose` naming the
//! one document revision it opens (`document_download:<document>@<revision>`).
//! The content route verifies it with
//! `verify_bearer(token, &[JwtAudience::Resource(DOWNLOAD_AUDIENCE)])`,
//! checks the purpose against the requested path and then authorizes the
//! read as the token's subject, acting as the agent the purpose names
//! (`;agent=<name>`), so the document's agent cap is checked again at use. A
//! `format=pdf` query on the same URL serves the revision rendered as PDF under
//! the same token.
//!
//! [`TokenAuthority`]: systemprompt_models::auth::TokenAuthority
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration, Utc};
use systemprompt_identifiers::{AccessTokenId, DocumentId, DocumentRevisionId, SessionId};
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, TokenAuthority, TokenError, UserJwtRequest,
};
use systemprompt_models::modules::ApiPaths;

pub const DOWNLOAD_AUDIENCE: &str = "documents.download";
pub const DOWNLOAD_TOKEN_QUERY: &str = "access_token";
pub const DOWNLOAD_FORMAT_QUERY: &str = "format";
pub const DOWNLOAD_FORMAT_PDF: &str = "pdf";
pub const DOWNLOAD_TTL: Duration = Duration::minutes(10);

#[must_use]
pub fn download_purpose(document: &DocumentId, revision: &DocumentRevisionId) -> String {
    format!("document_download:{document}@{revision}")
}

const AGENT_SUFFIX: &str = ";agent=";

/// The signed purpose of a link an agent minted: the revision it opens and
/// the agent's name, so the read at use runs under that agent's cap.
#[must_use]
pub fn agent_download_purpose(
    document: &DocumentId,
    revision: &DocumentRevisionId,
    agent: &str,
) -> String {
    format!(
        "{}{AGENT_SUFFIX}{agent}",
        download_purpose(document, revision)
    )
}

/// Whether `purpose` opens this revision, and the agent named in it.
/// `None` when it opens another revision; `Some(None)` for a link with no
/// agent (still read as an unnamed agent: a link is a bearer credential).
#[must_use]
pub fn download_agent<'a>(
    purpose: &'a str,
    document: &DocumentId,
    revision: &DocumentRevisionId,
) -> Option<Option<&'a str>> {
    let base = download_purpose(document, revision);
    let rest = purpose.strip_prefix(base.as_str())?;
    if rest.is_empty() {
        return Some(None);
    }
    rest.strip_prefix(AGENT_SUFFIX).map(Some)
}

/// Why a download link could not be signed.
#[derive(Debug, thiserror::Error)]
#[error("the download link could not be signed")]
pub struct DownloadLinkError(#[source] TokenError);

impl From<TokenError> for DownloadLinkError {
    fn from(error: TokenError) -> Self {
        Self(error)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendition {
    Native,
    Pdf,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadTarget<'a> {
    pub rendition: Rendition,
    pub base_url: &'a str,
    pub user: &'a AuthenticatedUser,
    pub session_id: &'a SessionId,
    pub document: &'a DocumentId,
    pub revision: &'a DocumentRevisionId,
    pub agent: &'a str,
}

pub fn signed_download_url(
    authority: &dyn TokenAuthority,
    target: DownloadTarget<'_>,
) -> Result<(String, DateTime<Utc>), DownloadLinkError> {
    let request = UserJwtRequest {
        user: target.user.clone(),
        session_id: target.session_id.clone(),
        jti: AccessTokenId::generate(),
        permissions: target.user.permissions().to_vec(),
        audience: vec![JwtAudience::Resource(DOWNLOAD_AUDIENCE.to_owned())],
        ttl: DOWNLOAD_TTL,
        client_id: None,
        purpose: Some(agent_download_purpose(
            target.document,
            target.revision,
            target.agent,
        )),
        plugin_id: None,
        interaction: None,
        browser_session: false,
    };
    let token = authority
        .mint_user_jwt(&request)
        .map_err(DownloadLinkError)?;
    let rendition = match target.rendition {
        Rendition::Native => String::new(),
        Rendition::Pdf => format!("&{DOWNLOAD_FORMAT_QUERY}={DOWNLOAD_FORMAT_PDF}"),
    };
    let url = format!(
        "{base}{path}/{document}/content?revision={revision}{rendition}&{DOWNLOAD_TOKEN_QUERY}={token}",
        base = target.base_url.trim_end_matches('/'),
        path = ApiPaths::DOCUMENTS,
        document = target.document,
        revision = target.revision,
    );
    Ok((url, Utc::now() + DOWNLOAD_TTL))
}
