//! The anonymous open of a public link.
//!
//! An open that does not resolve to a live link of an allowed, untrashed
//! document with the right password is refused with one undifferentiated
//! [`PublicLinkRefusal::Unavailable`]. Failed passwords are counted per link
//! and client address and refused with [`PublicLinkRefusal::Throttled`] once
//! [`MAX_FAILED_PASSWORDS`] have failed inside [`PASSWORD_WINDOW_MINUTES`].
//! The policy is re-checked on every open, so tightening it closes links
//! already handed out.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration, Utc};
use systemprompt_traits::{RepositoryError, StoredFileRead};

use super::DocumentService;
use super::external::{MAX_LINK_PASSWORD_BYTES, token_hash, well_formed_token};
use crate::config::ExternalChannel;
use crate::documents::error::DocumentsError;
use crate::documents::models::{Document, DocumentRevision};

pub const MAX_FAILED_PASSWORDS: i64 = 5;
pub const PASSWORD_WINDOW_MINUTES: i64 = 15;

const UNKNOWN_CLIENT: &str = "unknown";

/// What an anonymous open of a public link reads: the document, its head
/// revision and that revision's bytes.
#[derive(Debug)]
pub struct PublicLinkOpen {
    pub document: Document,
    pub revision: DocumentRevision,
    pub read: StoredFileRead,
    pub agent_content: bool,
}

/// Why an anonymous open was refused. `Unavailable` covers an unknown,
/// revoked or expired link, a policy that withholds it, a trashed document
/// and a missing or wrong password alike.
#[derive(Debug, thiserror::Error)]
pub enum PublicLinkRefusal {
    #[error("this link is unavailable")]
    Unavailable,
    #[error("too many password attempts; try again later")]
    Throttled,
    #[error("public link store")]
    Store(#[from] DocumentsError),
}

impl From<RepositoryError> for PublicLinkRefusal {
    fn from(source: RepositoryError) -> Self {
        Self::Store(source.into())
    }
}

impl DocumentService {
    pub async fn open_public_link(
        &self,
        token: &str,
        password: Option<&str>,
        client_ip: Option<std::net::IpAddr>,
    ) -> Result<PublicLinkOpen, PublicLinkRefusal> {
        if !well_formed_token(token) {
            return Err(PublicLinkRefusal::Unavailable);
        }
        let now = Utc::now();
        let gate = self
            .repos
            .public_links
            .gate(&token_hash(token))
            .await?
            .filter(|gate| gate.is_live(now))
            .ok_or(PublicLinkRefusal::Unavailable)?;
        let document = self
            .repos
            .documents
            .find(&gate.document_id)
            .await?
            .ok_or(PublicLinkRefusal::Unavailable)?;
        if !self
            .external_allowed(ExternalChannel::PublicLink, &document)
            .await?
        {
            return Err(PublicLinkRefusal::Unavailable);
        }
        if let Some(stored) = gate.password_hash.as_deref() {
            let client = client_ip.map_or_else(|| UNKNOWN_CLIENT.to_owned(), |ip| ip.to_string());
            let window = now - Duration::minutes(PASSWORD_WINDOW_MINUTES);
            let links = &self.repos.public_links;
            if links.failed_attempts(&gate.id, &client, window).await? >= MAX_FAILED_PASSWORDS {
                return Err(PublicLinkRefusal::Throttled);
            }
            let Some(password) = password else {
                return Err(PublicLinkRefusal::Unavailable);
            };
            if !verify_password(password.to_owned(), stored.to_owned()).await? {
                links.record_failure(&gate.id, &client, window).await?;
                return Err(PublicLinkRefusal::Unavailable);
            }
        }
        let revision = self.resolve_revision(&document, None).await?;
        let read = self
            .blobs
            .open(&revision.content_sha256, None)
            .await
            .map_err(DocumentsError::from)?;
        let agent_content = self.agent_content(&document.id).await?;
        self.repos.public_links.record_open(&gate.id).await?;
        Ok(PublicLinkOpen {
            document,
            revision,
            read,
            agent_content,
        })
    }
}

async fn verify_password(password: String, stored: String) -> Result<bool, DocumentsError> {
    if password.len() > MAX_LINK_PASSWORD_BYTES {
        return Ok(false);
    }
    tokio::task::spawn_blocking(move || bcrypt::verify(password, &stored))
        .await?
        .map_err(|source| DocumentsError::LinkPassword { source })
}
