//! The `meta` read view: the document's name, place, access,
//! head revision, size and who is in it now, without its content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentUri;

use super::types::{DocumentMeta, EditingNow, ReadInput, ReadOutput};

pub(super) async fn meta(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: &ReadInput,
    trace: &TraceId,
) -> Result<ReadOutput, DocumentsError> {
    let view = service.get(who, &input.document_id, trace).await?;
    let head = view.head.ok_or(DocumentsError::InvalidParams {
        detail: "the document has no revision yet",
    })?;
    let size_bytes = service.revision_size(&head).await?;
    let editing: Vec<EditingNow> = service
        .presence(who, &input.document_id, trace)
        .await?
        .into_iter()
        .map(|row| EditingNow {
            user_id: row.user_id,
            source: row.source,
            state: row.state,
        })
        .collect();
    Ok(ReadOutput {
        uri: input
            .verbose
            .then(|| DocumentUri::new(view.document.id.clone()).at_revision(head.id.clone())),
        revision_id: head.id,
        content_sha256: input.verbose.then_some(head.content_sha256),
        format: view.document.format,
        lines: Vec::new(),
        truncated: false,
        model: None,
        meta: Some(DocumentMeta {
            name: view.document.name,
            folder_path: view.folder_path,
            access: view.my_access,
            size_bytes,
            updated_at: view.document.updated_at,
            editing,
        }),
        paths: Vec::new(),
    })
}
