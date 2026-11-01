use serde_json::json;
use systemprompt_identifiers::{DocumentId, DocumentTemplateId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{AccessLevel, ActorKind, DocumentFormat};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    CreateHandler, CreateInput, DocumentsMcpTools, HistoryHandler, HistoryInput, ListHandler,
    ListInput, ReadHandler, SearchHandler, SearchInput,
};
use systemprompt_runtime::documents::refusal::RefusalCode;
use systemprompt_runtime::documents::views::{ReadInput, ReadOutput, ReadView};

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, registered};

async fn create_markdown(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    markdown: &str,
) -> DocumentId {
    CreateHandler { tools }
        .handle(
            CreateInput {
                template_id: DocumentTemplateId::new("blank-markdown"),
                folder_id: None,
                name: format!("md-{}", uuid::Uuid::new_v4().simple()),
                params: Default::default(),
                content: Some(json!({ "markdown": markdown })),
                ops: Vec::new(),
                message: None,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("create markdown")
        .0
        .document_id
}

async fn model(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    document: &DocumentId,
    verbose: bool,
) -> Result<ReadOutput, rmcp::ErrorData> {
    let input = ReadInput {
        document_id: document.clone(),
        view: ReadView::Model,
        range: None,
        sheet: None,
        cells: None,
        revision_id: None,
        since_revision: None,
        node_id: None,
        text: None,
        verbose,
    };
    ReadHandler { tools }
        .handle(input, ctx, &exec())
        .await
        .map(|(output, _)| output)
}

#[tokio::test]
async fn a_model_read_answers_the_rich_text_model() {
    let r = rig(false).await;
    let user = registered("model").await;
    let ctx = ctx_for(Some(&user));
    let document = create_markdown(&r.tools, &ctx, "hello world\n").await;
    let read = model(&r.tools, &ctx, &document, true).await.expect("model");
    assert_eq!(read.format, DocumentFormat::Markdown);
    assert!(read.lines.is_empty());
    assert!(read.uri.is_some() && read.content_sha256.is_some());
    let view = read.model.expect("model view");
    let blocks = view["blocks"].as_array().expect("blocks");
    assert!(!blocks.is_empty(), "{view}");
    assert_eq!(view["kind"], "rich_text");
    assert_eq!(blocks[0]["type"], "paragraph");
    assert_eq!(blocks[0]["text"], "hello world");
    assert!(blocks[0]["id"].as_str().is_some_and(|id| !id.is_empty()));
    let terse = model(&r.tools, &ctx, &document, false)
        .await
        .expect("terse");
    assert!(terse.uri.is_none() && terse.content_sha256.is_none());
}

#[tokio::test]
async fn a_model_read_of_plain_text_is_refused() {
    let r = rig(false).await;
    let user = registered("model-text").await;
    let ctx = ctx_for(Some(&user));
    let (document, _) = create_text(&r.tools, &ctx, "alpha").await;
    let error = model(&r.tools, &ctx, &document, false)
        .await
        .expect_err("text has no model");
    assert_eq!(code_of(&error), RefusalCode::InvalidParams);
    assert!(error.message.contains("Markdown"), "{}", error.message);
}

#[tokio::test]
async fn mine_lists_the_callers_documents_with_their_access() {
    let r = rig(false).await;
    let user = registered("list-mine").await;
    let ctx = ctx_for(Some(&user));
    let (document, _) = create_text(&r.tools, &ctx, "alpha").await;
    let input: ListInput =
        serde_json::from_value(json!({"scope": "mine", "sort": "name", "verbose": true}))
            .expect("list input");
    let (listing, summary) = ListHandler { tools: &r.tools }
        .handle(input, &ctx, &exec())
        .await
        .expect("list");
    let item = listing
        .items
        .iter()
        .find(|item| item.document_id == document)
        .expect("listed");
    assert_eq!(item.my_access, Some(AccessLevel::Own));
    assert!(item.uri.is_some());
    assert!(summary.ends_with(&format!("{} documents", listing.items.len())));
}

#[tokio::test]
async fn a_malformed_cursor_is_refused() {
    let r = rig(false).await;
    let user = registered("list-cursor").await;
    let input: ListInput =
        serde_json::from_value(json!({"cursor": "garbage"})).expect("list input");
    let error = ListHandler { tools: &r.tools }
        .handle(input, &ctx_for(Some(&user)), &exec())
        .await
        .expect_err("cursor");
    assert_eq!(code_of(&error), RefusalCode::InvalidParams);
    assert!(error.message.contains("without a cursor"));
}

#[tokio::test]
async fn search_filters_hits_by_format() {
    let r = rig(false).await;
    let user = registered("search").await;
    let ctx = ctx_for(Some(&user));
    let word = format!("zebra{}", uuid::Uuid::new_v4().simple());
    let (text_doc, _) = create_text(&r.tools, &ctx, &word).await;
    let md_doc = create_markdown(&r.tools, &ctx, &format!("{word}\n")).await;
    let search = |format: Option<&str>| -> SearchInput {
        serde_json::from_value(json!({"query": word, "format": format})).expect("search")
    };
    let handler = SearchHandler { tools: &r.tools };
    let (all, _) = handler
        .handle(search(None), &ctx, &exec())
        .await
        .expect("search");
    let found: Vec<_> = all.hits.iter().map(|h| h.document_id.clone()).collect();
    assert!(
        found.contains(&text_doc) && found.contains(&md_doc),
        "{found:?}"
    );
    let (markdown, summary) = handler
        .handle(search(Some("markdown")), &ctx, &exec())
        .await
        .expect("filtered");
    assert!(
        markdown
            .hits
            .iter()
            .all(|h| h.format == DocumentFormat::Markdown)
    );
    assert!(markdown.hits.iter().any(|h| h.document_id == md_doc));
    assert_eq!(
        summary,
        format!("{} matching documents", markdown.hits.len())
    );
}

#[tokio::test]
async fn verbose_history_carries_uris_hashes_and_attribution() {
    let r = rig(false).await;
    let user = registered("history").await;
    let ctx = ctx_for(Some(&user));
    let (document, head) = create_text(&r.tools, &ctx, "alpha").await;
    let history = |verbose: bool| HistoryInput {
        document_id: document.clone(),
        limit: Some(10),
        verbose,
    };
    let handler = HistoryHandler { tools: &r.tools };
    let (verbose, summary) = handler
        .handle(history(true), &ctx, &exec())
        .await
        .expect("history");
    assert_eq!(summary, format!("{} revisions", verbose.revisions.len()));
    let newest = &verbose.revisions[0];
    assert_eq!(newest.revision_id, head);
    assert_eq!(newest.actor_kind, ActorKind::Agent);
    assert_eq!(newest.message.as_deref(), Some("seed"));
    assert!(newest.uri.is_some() && newest.content_sha256.is_some());
    assert!(newest.parent_revision_id.is_some());
    let attribution = newest.attribution.as_ref().expect("attribution");
    assert_eq!(attribution.trace_id.as_ref(), Some(ctx.trace_id()));
    let (terse, _) = handler
        .handle(history(false), &ctx, &exec())
        .await
        .expect("terse");
    assert!(terse.revisions.iter().all(|r| r.uri.is_none()
        && r.content_sha256.is_none()
        && r.parent_revision_id.is_none()
        && r.attribution.is_none()));
}
