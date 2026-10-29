//! [`CollabError`]: the error surface of the live co-editing service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentEditSessionId, DocumentId};
use systemprompt_traits::RepositoryError;

use super::document::ModelError;
use crate::documents::blob_store::BlobStoreError;
use crate::documents::engine::EngineError;
use crate::documents::error::DocumentsError;
use crate::documents::model::ModelRejection;

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
    #[error("the update is not a valid update of the live session engine")]
    InvalidUpdate(#[source] EngineError),
    #[error("the update was rejected: {0}")]
    UpdateRejected(#[source] ModelRejection),
    #[error("the version is not a valid version of the live session engine")]
    InvalidVersion(#[source] EngineError),
    #[error("no live session engine is enabled on this installation")]
    EngineUnavailable,
    #[error("document {document_id} is not UTF-8 text and cannot be co-edited")]
    NotText {
        document_id: DocumentId,
        #[source]
        source: std::string::FromUtf8Error,
    },
    #[error("document {document_id} could not be read or written as its live model")]
    Model {
        document_id: DocumentId,
        #[source]
        source: ModelError,
    },
    #[error("edit session state could not be updated")]
    Engine(#[from] EngineError),
    #[error("edit session store query failed")]
    Repository(#[from] RepositoryError),
    #[error("edit session snapshot storage failed")]
    Blob(#[from] BlobStoreError),
}
