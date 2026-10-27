//! Read-only listing tools: `documents_list`, `documents_search` and
//! `documents_history`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use rmcp::ErrorData as McpError;
use systemprompt_files::documents::models::{Document, ListCursor};
use systemprompt_files::documents::service::{HistoryPage, ListFolderRequest};
use systemprompt_identifiers::McpExecutionId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::DocumentUri;
use systemprompt_models::execution::context::RequestContext;

use super::DocumentsMcpTools;
use super::inputs::{HistoryInput, ListInput, SearchInput};
use super::outputs::{
    DocumentItem, FolderItem, HistoryOutput, ListOutput, RevisionItem, SearchHit, SearchOutput,
};
use super::support::{caller, clamp_limit, tool_error};

fn document_uri(document: &Document) -> DocumentUri {
    let uri = DocumentUri::new(document.id.clone());
    match &document.head_revision_id {
        Some(revision) => uri.at_revision(revision.clone()),
        None => uri,
    }
}

fn document_item(document: Document) -> DocumentItem {
    DocumentItem {
        uri: document_uri(&document),
        document_id: document.id,
        folder_id: document.folder_id,
        name: document.name,
        format: document.format,
        head_revision_id: document.head_revision_id,
        updated_at: document.updated_at,
    }
}

fn name_matches(name: &str, query: Option<&str>) -> bool {
    query
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .is_none_or(|q| name.to_lowercase().contains(&q.to_lowercase()))
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
        "List the folders and documents in one folder (the root when folder_id is omitted) that \
         you may view. query narrows by name; pass next_cursor back as cursor for the next page."
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
                raw.parse::<ListCursor>().map_err(|error| {
                    tracing::debug!(error = %error, cursor = raw, "list cursor rejected");
                    McpError::invalid_params(
                        format!("cursor \"{raw}\" is not valid; list again without a cursor"),
                        None,
                    )
                })
            })
            .transpose()?;
        let listing = self
            .tools
            .service()
            .list_folder(
                who,
                ListFolderRequest {
                    folder: input.folder_id,
                    cursor,
                    limit: clamp_limit(input.limit),
                },
                ctx.trace_id(),
            )
            .await
            .map_err(tool_error)?;
        let query = input.query.as_deref();
        let output = ListOutput {
            folders: listing
                .folders
                .into_iter()
                .filter(|f| name_matches(&f.name, query))
                .map(|f| FolderItem {
                    folder_id: f.id,
                    parent_id: f.parent_id,
                    name: f.name,
                })
                .collect(),
            items: listing
                .documents
                .into_iter()
                .filter(|d| name_matches(&d.name, query))
                .map(document_item)
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
        "Full-text search over the head revision of every document you may view. Each hit \
         carries a snippet and a document:// URI to read next."
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
                uri: DocumentUri::new(r.document.id.clone()).at_revision(r.hit.revision_id.clone()),
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
        "List a document's revisions, newest first, with author, actor kind and message."
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
        let revisions: Vec<RevisionItem> = revisions
            .into_iter()
            .map(|r| RevisionItem {
                uri: DocumentUri::new(r.document_id.clone()).at_revision(r.id.clone()),
                revision_id: r.id,
                parent_revision_id: r.parent_revision_id,
                author_user_id: r.author_user_id,
                actor_kind: r.actor_kind,
                message: r.message,
                content_sha256: r.content_sha256,
                created_at: r.created_at,
            })
            .collect();
        let summary = format!("{} revisions", revisions.len());
        Ok((HistoryOutput { revisions }, summary))
    }
}
