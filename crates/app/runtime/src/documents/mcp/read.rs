//! Reads hand back hashlines so every later edit can quote a node id and
//! hash it has already seen. Node ids are stable across revisions, so a
//! `changes` read since an earlier revision is enough to resume on a
//! document someone else edited.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::DocumentService;
use systemprompt_files::documents::nodes::{changes, path_of};
use systemprompt_identifiers::{McpExecutionId, TraceId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{DocumentUri, Projection};
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{ReadInput, ReadView};
use super::lines::{MAX_LINES, RenderedLines, render, shows, truncate};
use super::outputs::ReadOutput;
use super::read_meta::meta;
use super::support::{caller, tool_error};
use super::views::change_lines;

async fn changed(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: &ReadInput,
    trace: &TraceId,
    head: &Projection,
) -> Result<RenderedLines, McpError> {
    let since = input.since_revision.as_ref().ok_or_else(|| {
        McpError::invalid_params(
            "view changes needs since_revision: the revision_id of your last read or write",
            None,
        )
    })?;
    let (_, before) = service
        .project(who, &input.document_id, Some(since), trace)
        .await
        .map_err(tool_error)?;
    let diff = changes(&before, head, shows);
    Ok(truncate(change_lines(head, &diff)))
}

pub async fn read_view(
    service: &DocumentService,
    who: &AuthenticatedUser,
    input: ReadInput,
    trace: &TraceId,
) -> Result<(ReadOutput, String), McpError> {
    if input.view == ReadView::Meta {
        return meta(service, who, &input, trace).await;
    }
    let (revision, projection) = service
        .project(who, &input.document_id, input.revision_id.as_ref(), trace)
        .await
        .map_err(tool_error)?;
    let rendered = if input.view == ReadView::Changes {
        changed(service, who, &input, trace, &projection).await?
    } else {
        render(&projection, &input, &revision.id)?
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
    let summary = format!(
        "{} lines of revision {}{}",
        rendered.lines.len(),
        revision.id,
        if rendered.truncated {
            " (truncated)"
        } else {
            ""
        }
    );
    Ok((
        ReadOutput {
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
        },
        summary,
    ))
}

#[derive(Debug, Clone, Copy)]
pub struct ReadHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ReadHandler<'_> {
    type Input = ReadInput;
    type Output = ReadOutput;

    fn tool_name(&self) -> &'static str {
        "documents_read"
    }

    fn description(&self) -> &'static str {
        "Read `<node>:<hash>| <text>` lines; view outline first, then text, range, node, cells, \
         search, changes or meta. Guide topic: editing."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: ReadInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ReadOutput, String), McpError> {
        let who = caller(ctx)?;
        read_view(self.tools.service(), who, input, ctx.trace_id()).await
    }
}
