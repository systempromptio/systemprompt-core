//! [`CollabError`]: the error surface of the live co-editing service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use loro::{LoroEncodeError, LoroError};
use systemprompt_identifiers::{DocumentEditSessionId, DocumentId};
use systemprompt_traits::RepositoryError;

use crate::documents::blob_store::BlobStoreError;
use crate::documents::error::DocumentsError;

#[derive(Debug, thiserror::Error)]
pub enum CollabError {
    #[error(transparent)]
    Documents(#[from] DocumentsError),
    #[error(
        "edit session {session_id} of document {document_id} is no longer open; join the \
         document again"
    )]
    SessionClosed {
        document_id: DocumentId,
        session_id: DocumentEditSessionId,
    },
    #[error("join document {document_id} in edit mode before sending updates")]
    NotEditing { document_id: DocumentId },
    #[error("join document {document_id} before sending a cursor")]
    NotJoined { document_id: DocumentId },
    #[error("the update is not a valid Loro update")]
    InvalidUpdate(#[source] LoroError),
    #[error("the version vector is not a valid Loro version vector")]
    InvalidVersion(#[source] LoroError),
    #[error("document {document_id} is not UTF-8 text and cannot be co-edited")]
    NotText {
        document_id: DocumentId,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("edit session state could not be updated")]
    Crdt(#[source] LoroError),
    #[error("edit session state could not be encoded")]
    Encode(#[from] LoroEncodeError),
    #[error("edit session text could not be reconciled with the merged revision")]
    Reconcile(#[source] loro::UpdateTimeoutError),
    #[error("edit session store query failed")]
    Repository(#[from] RepositoryError),
    #[error("edit session snapshot storage failed")]
    Blob(#[from] BlobStoreError),
}
