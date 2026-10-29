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
    #[error(
        "edit session {session_id} of document {document_id} is being checkpointed by another \
         replica"
    )]
    CheckpointBusy {
        document_id: DocumentId,
        session_id: DocumentEditSessionId,
    },
    #[error(
        "the checkpoint lease of edit session {session_id} of document {document_id} expired \
         before the checkpoint was recorded"
    )]
    CheckpointLeaseLost {
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
    #[error(
        "the update carries operations of engine peer {peer}, which this caller was not \
         assigned; join the document again"
    )]
    PeerNotAssigned { peer: u64 },
    #[error(
        "the update depends on operations the edit session does not hold; join the document \
         again"
    )]
    MissingDependencies,
    #[error(
        "updates to document {document_id} kept landing on other replicas; send the update \
         again"
    )]
    UpdateContended { document_id: DocumentId },
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
    #[error("the live session's image owners are unreadable")]
    ImageOwners(#[from] super::image_owners::ImageOwnersError),
    #[error("edit session state could not be updated")]
    Engine(#[from] EngineError),
    #[error("edit session store query failed")]
    Repository(#[from] RepositoryError),
    #[error("edit session snapshot storage failed")]
    Blob(#[from] BlobStoreError),
}

pub(super) fn model_error(document: &DocumentId) -> impl FnOnce(ModelError) -> CollabError {
    let document_id = document.clone();
    move |source| CollabError::Model {
        document_id,
        source,
    }
}

pub(super) fn utf8(document: &DocumentId, bytes: Vec<u8>) -> Result<String, CollabError> {
    String::from_utf8(bytes).map_err(|source| CollabError::NotText {
        document_id: document.clone(),
        source,
    })
}
