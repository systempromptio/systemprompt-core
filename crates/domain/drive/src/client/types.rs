//! The values the Drive client's calls take and answer beside the protocol
//! types: a listing query, rendered pages and document bytes, the read view, an
//! edit's outcome and a local file to import.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use systemprompt_identifiers::{ContextId, DocumentFolderId};
use systemprompt_models::documents::client::{
    DriveLanded, DriveProposal, DriveProposed, DriveRevision, DriveScope,
};

/// What `GET /api/v1/documents` lists, one page at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub scope: DriveScope,
    pub folder: Option<DocumentFolderId>,
    pub query: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self {
            scope: DriveScope::Mine,
            folder: None,
            query: None,
            sort: None,
            limit: None,
            cursor: None,
        }
    }
}

/// An HTML answer (`rendered` or `preview`): the page, the policy the instance
/// served it under, and what it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPage {
    pub html: String,
    pub content_security_policy: Option<String>,
    pub revision: Option<String>,
    pub sha256: Option<String>,
}

/// A document's bytes in one of the formats the instance serves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentBytes {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub revision: Option<String>,
    pub sha256: Option<String>,
}

/// Which bytes [`DriveClient::content`](crate::DriveClient::content) asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFormat {
    Stored,
    Pdf,
    Markdown,
    Docx,
}

/// Which view of a document's nodes `GET /{id}/read` answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadView {
    Outline,
    Text,
}

impl ReadView {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Outline => "outline",
            Self::Text => "text",
        }
    }
}

/// What a structured edit did: landed as a revision, or left as a proposal for
/// a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditOutcome {
    Landed(Box<DriveLanded>),
    Proposed(Box<DriveProposed>),
}

impl EditOutcome {
    #[must_use]
    pub const fn revision(&self) -> &DriveRevision {
        match self {
            Self::Landed(landed) => &landed.revision,
            Self::Proposed(proposed) => &proposed.revision,
        }
    }

    #[must_use]
    pub const fn proposal(&self) -> Option<&DriveProposal> {
        match self {
            Self::Landed(_) => None,
            Self::Proposed(proposed) => Some(&proposed.proposal),
        }
    }
}

/// A local file to make a document of.
#[derive(Debug, Clone)]
pub struct ImportFile {
    pub name: String,
    pub bytes: Bytes,
    pub mime: Option<String>,
    pub folder_id: Option<DocumentFolderId>,
    pub message: Option<String>,
    pub context_id: Option<ContextId>,
}
