//! The read views: hashlines every later edit can quote a node id and hash
//! from. Node ids are stable across revisions, so a `changes` read since an
//! earlier revision is enough to resume on a document someone else edited.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_files::documents::nodes::{changes, path_of};
use systemprompt_files::documents::{DocumentService, DocumentsError};
use systemprompt_identifiers::TraceId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentUri, Projection};

use super::lines::{MAX_LINES, RenderedLines, render, shows, truncate};
use super::meta::meta;
use super::model::model_view;
use super::outline::change_lines;
use super::types::{ReadInput, ReadOutput, ReadView};

async fn changed(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: &ReadInput,
    trace: &TraceId,
    head: &Projection,
) -> Result<RenderedLines, DocumentsError> {
    let since = input
        .since_revision
        .as_ref()
        .ok_or(DocumentsError::InvalidParams {
            detail: "view changes needs since_revision: the revision_id of your last read or \
                     write",
        })?;
    let (_, before) = service
        .project(who, &input.document_id, Some(since), trace)
        .await?;
    let diff = changes(&before, head, shows);
    Ok(truncate(change_lines(head, &diff)))
}

pub async fn read_view(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: ReadInput,
    trace: &TraceId,
) -> Result<ReadOutput, DocumentsError> {
    if input.view == ReadView::Meta {
        return meta(service, who, &input, trace).await;
    }
    if input.view == ReadView::Model {
        return model_view(service, who, &input, trace).await;
    }
    let (revision, projection) = service
        .project(who, &input.document_id, input.revision_id.as_ref(), trace)
        .await?;
    let rendered = if input.view == ReadView::Changes {
        changed(service, who, &input, trace, &projection).await?
    } else {
        render(&projection, &input)?
    };
    let paths = if input.verbose {
        projection
            .nodes()
            .map(|node| format!("{} {}", node.id, path_of(node)))
            .take(MAX_LINES)
            .collect()
    } else {
        Vec::new()
    };
    Ok(ReadOutput {
        uri: input
            .verbose
            .then(|| DocumentUri::new(input.document_id).at_revision(revision.id.clone())),
        revision_id: revision.id,
        content_sha256: input.verbose.then_some(projection.content_sha256),
        format: projection.format,
        lines: rendered.lines,
        truncated: rendered.truncated,
        meta: None,
        paths,
        model: None,
    })
}
