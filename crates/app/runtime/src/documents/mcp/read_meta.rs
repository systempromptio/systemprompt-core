//! `documents_read` with `view: meta`: the document's name, place, access,
//! head revision, size and who is in it now, without its content.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::DocumentService;
use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::DocumentUri;

use super::inputs::ReadInput;
use super::outputs::{DocumentMeta, EditingNow, ReadOutput};
use super::support::tool_error;

pub(super) async fn meta(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: &ReadInput,
    trace: &TraceId,
) -> Result<(ReadOutput, String), McpError> {
    let view = service
        .get(who, &input.document_id, trace)
        .await
        .map_err(tool_error)?;
    let head = view
        .head
        .ok_or_else(|| McpError::invalid_params("the document has no revision yet", None))?;
    let size_bytes = service.revision_size(&head).await.map_err(tool_error)?;
    let editing: Vec<EditingNow> = service
        .presence(who, &input.document_id, trace)
        .await
        .map_err(tool_error)?
        .into_iter()
        .map(|row| EditingNow {
            user_id: row.user_id,
            source: row.source,
            state: row.state,
        })
        .collect();
    let summary = format!(
        "{} at revision {}, {} present",
        view.document.name,
        head.id,
        editing.len()
    );
    Ok((
        ReadOutput {
            uri: input
                .verbose
                .then(|| DocumentUri::new(view.document.id.clone()).at_revision(head.id.clone())),
            revision_id: head.id,
            content_sha256: input.verbose.then_some(head.content_sha256),
            format: view.document.format,
            lines: Vec::new(),
            truncated: false,
            meta: Some(DocumentMeta {
                name: view.document.name,
                folder_path: view.folder_path,
                access: view.my_access,
                size_bytes,
                updated_at: view.document.updated_at,
                editing,
            }),
            paths: Vec::new(),
        },
        summary,
    ))
}
