//! Read-only listing tools: `documents_list`, `documents_search` and
//! `documents_history`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use rmcp::ErrorData as McpError;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_files::documents::models::{Document, ListedDocument, ListingCursor};
use systemprompt_files::documents::service::{HistoryPage, ListFolderRequest};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_models::documents::DocumentUri;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{HistoryInput, ListInput, ListScopeInput, SearchInput};
use super::outputs::{
    DocumentItem, FolderItem, HistoryOutput, ListOutput, RevisionAttributionItem, RevisionItem,
    SearchHit, SearchOutput,
};
use super::support::{caller, clamp_limit, tool_error};
use crate::attribution::{attributed_context, owned_context_names};

fn document_uri(document: &Document) -> DocumentUri {
    let uri = DocumentUri::new(document.id.clone());
    match &document.head_revision_id {
        Some(revision) => uri.at_revision(revision.clone()),
        None => uri,
    }
}

pub(super) fn document_item(document: Document, verbose: bool) -> DocumentItem {
    DocumentItem {
        uri: verbose.then(|| document_uri(&document)),
        document_id: document.id,
        folder_id: document.folder_id,
        name: document.name,
        format: document.format,
        head_revision_id: document.head_revision_id,
        updated_at: document.updated_at,
        size_bytes: None,
        my_access: None,
    }
}

fn listed_item(listed: ListedDocument, verbose: bool) -> DocumentItem {
    DocumentItem {
        size_bytes: listed.size_bytes,
        my_access: Some(listed.my_access),
        ..document_item(listed.document, verbose)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ListHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for ListHandler<'_> {
    type Input = ListInput;
    type Output = ListOutput;

    fn tool_name(&self) -> &'static str {
        "documents_list"
    }

    fn description(&self) -> &'static str {
        "List documents by scope."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: ListInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(ListOutput, String), McpError> {
        let who = caller(ctx)?;
        let cursor = input
            .cursor
            .as_deref()
            .map(|raw| {
                raw.parse::<ListingCursor>().map_err(|error| {
                    tracing::debug!(error = %error, cursor = raw, "list cursor rejected");
                    McpError::invalid_params(
                        format!("cursor \"{raw}\" is not valid; list again without a cursor"),
                        None,
                    )
                })
            })
            .transpose()?;
        let context_id = if input.scope == ListScopeInput::Context {
            let claimed = input
                .context_id
                .as_ref()
                .unwrap_or_else(|| ctx.context_id());
            attributed_context(self.tools.contexts().as_ref(), ctx, Some(claimed))
                .await
                .map_err(tool_error)?
        } else {
            None
        };
        let listing = self
            .tools
            .service()
            .list_folder(
                who,
                ListFolderRequest {
                    scope: input.scope.scope(),
                    folder: input.folder_id,
                    context_id,
                    query: input.query,
                    sort: input.sort.sort(),
                    cursor,
                    limit: clamp_limit(input.limit),
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let output = ListOutput {
            folders: listing
                .folders
                .into_iter()
                .map(|f| FolderItem {
                    folder_id: f.id,
                    parent_id: f.parent_id,
                    name: f.name,
                })
                .collect(),
            items: listing
                .documents
                .into_iter()
                .map(|d| listed_item(d, input.verbose))
                .collect(),
            next_cursor: listing.next_cursor.map(|c| c.to_string()),
        };
        let summary = format!(
            "{} folders and {} documents",
            output.folders.len(),
            output.items.len()
        );
        Ok((output, summary))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SearchHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for SearchHandler<'_> {
    type Input = SearchInput;
    type Output = SearchOutput;

    fn tool_name(&self) -> &'static str {
        "documents_search"
    }

    fn description(&self) -> &'static str {
        "Search documents you may view."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: SearchInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(SearchOutput, String), McpError> {
        let who = caller(ctx)?;
        let results = self
            .tools
            .service()
            .search(who, &input.query, clamp_limit(input.limit), ctx.trace_id())
            .await
            .map_err(tool_error)?;
        let hits: Vec<SearchHit> = results
            .into_iter()
            .filter(|r| input.format.is_none_or(|f| f == r.document.format))
            .map(|r| SearchHit {
                document_id: r.document.id,
                revision_id: r.hit.revision_id,
                name: r.document.name,
                format: r.document.format,
                snippet: r.hit.snippet,
            })
            .collect();
        let summary = format!("{} matching documents", hits.len());
        Ok((SearchOutput { hits }, summary))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HistoryHandler<'a> {
    pub tools: &'a DocumentsMcpTools,
}

impl McpToolHandler for HistoryHandler<'_> {
    type Input = HistoryInput;
    type Output = HistoryOutput;

    fn tool_name(&self) -> &'static str {
        "documents_history"
    }

    fn description(&self) -> &'static str {
        "Revisions, newest first."
    }

    fn read_only(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        input: HistoryInput,
        ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(HistoryOutput, String), McpError> {
        let who = caller(ctx)?;
        let revisions = self
            .tools
            .service()
            .history(
                who,
                &input.document_id,
                HistoryPage {
                    limit: clamp_limit(input.limit),
                    before: None,
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let verbose = input.verbose;
        let names = if verbose {
            owned_context_names(
                self.tools.contexts().as_ref(),
                &who.id,
                revisions
                    .iter()
                    .filter_map(|r| r.attribution.context_id.as_ref()),
            )
            .await
            .map_err(tool_error)?
        } else {
            HashMap::new()
        };
        let revisions: Vec<RevisionItem> = revisions
            .into_iter()
            .map(|r| RevisionItem {
                uri: verbose
                    .then(|| DocumentUri::new(r.document_id.clone()).at_revision(r.id.clone())),
                revision_id: r.id,
                parent_revision_id: r.parent_revision_id.filter(|_| verbose),
                author_user_id: r.author_user_id,
                actor_kind: r.actor_kind,
                message: r.message,
                content_sha256: verbose.then_some(r.content_sha256),
                attribution: verbose.then(|| RevisionAttributionItem {
                    context_name: r
                        .attribution
                        .context_id
                        .as_ref()
                        .and_then(|id| names.get(id).cloned()),
                    session_id: r.attribution.session_id,
                    context_id: r.attribution.context_id,
                    trace_id: r.attribution.trace_id,
                    task_id: r.attribution.task_id,
                    on_behalf_of: r.attribution.on_behalf_of,
                    agent_name: r.attribution.agent_name,
                    client_id: r.attribution.client_id,
                }),
                created_at: r.created_at,
            })
            .collect();
        let summary = format!("{} revisions", revisions.len());
        Ok((HistoryOutput { revisions }, summary))
    }
}
