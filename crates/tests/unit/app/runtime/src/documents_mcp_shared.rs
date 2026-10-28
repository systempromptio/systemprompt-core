//! The shared-editing surface of the documents tools: reads of what changed
//! since a revision, search and meta views, the folded outline and node
//! paging, op id replay, and the attribution an agent's revision records.

use std::collections::BTreeMap;

use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentOpId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{
    ContentAs, DocumentFormat, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
    content_hash, hash8,
};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    DocumentsMcpTools, EditHandler, EditInput, HistoryHandler, HistoryInput, ReadHandler,
};
use systemprompt_runtime::documents::views::{
    EditOutput, ReadInput, ReadOutput, ReadView, node_lines, outline, search_lines,
};

use crate::documents_mcp_support::{
    create_text, ctx_for, exec, harness, line_for, read_text, registered,
};

fn read_input(document: &DocumentId, view: ReadView) -> ReadInput {
    ReadInput {
        document_id: document.clone(),
        view,
        range: None,
        sheet: None,
        cells: None,
        revision_id: None,
        since_revision: None,
        node_id: None,
        text: None,
        verbose: false,
    }
}

async fn read(tools: &DocumentsMcpTools, ctx: &RequestContext, input: ReadInput) -> ReadOutput {
    ReadHandler { tools }
        .handle(input, ctx, &exec())
        .await
        .expect("read")
        .0
}

async fn replace_line(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    document: &DocumentId,
    from: &str,
    to: &str,
    op_id: Option<DocumentOpId>,
) -> EditOutput {
    let current = read_text(tools, ctx, document).await;
    let (node, hash) = line_for(&current, from);
    EditHandler { tools }
        .handle(
            EditInput {
                document_id: document.clone(),
                base_revision: None,
                mode: None,
                op_id,
                answering: None,
                allow_detach: false,
                ops: vec![EditOp::ReplaceText {
                    node: NodeTarget::Id(DocumentNodeId::new(node)),
                    expected_hash: Some(hash.to_owned()),
                    text: to.to_owned(),
                    content_as: ContentAs::Text,
                }],
                message: None,
                verbose: false,
            },
            ctx,
            &exec(),
        )
        .await
        .expect("edit")
        .0
}

#[tokio::test]
async fn changes_since_a_revision_return_only_what_changed() {
    let h = harness().await;
    let owner = registered("since").await;
    let ctx = ctx_for(Some(&owner));
    let (document, first) = create_text(&h.tools, &ctx, "alpha\nbeta\ngamma\ndelta").await;
    let before = read_text(&h.tools, &ctx, &document).await;
    let (beta_id, _) = line_for(&before, "beta");
    replace_line(&h.tools, &ctx, &document, "beta", "BETA", None).await;
    let mut input = read_input(&document, ReadView::Changes);
    input.since_revision = Some(first);
    let changed = read(&h.tools, &ctx, input).await;
    assert!(
        changed
            .lines
            .iter()
            .any(|line| line.starts_with(beta_id) && line.ends_with("| BETA")),
        "{:?}",
        changed.lines
    );
    assert!(
        changed
            .lines
            .iter()
            .any(|line| line.starts_with("gap blocks="))
    );
    assert!(!changed.lines.iter().any(|line| line.ends_with("| alpha")));
}

fn edit_input(document: &DocumentId, op_id: &DocumentOpId, op: EditOp) -> EditInput {
    EditInput {
        document_id: document.clone(),
        base_revision: None,
        mode: None,
        op_id: Some(op_id.clone()),
        answering: None,
        allow_detach: false,
        ops: vec![op],
        message: None,
        verbose: false,
    }
}

#[tokio::test]
async fn a_repeated_op_id_replays_the_first_ack() {
    let h = harness().await;
    let owner = registered("replay").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "one\ntwo").await;
    let op_id = DocumentOpId::new(format!("op-{}", uuid::Uuid::new_v4()));
    let current = read_text(&h.tools, &ctx, &document).await;
    let (node, hash) = line_for(&current, "two");
    let op = EditOp::ReplaceText {
        node: NodeTarget::Id(DocumentNodeId::new(node)),
        expected_hash: Some(hash.to_owned()),
        text: "TWO".to_owned(),
        content_as: ContentAs::Text,
    };
    let handler = EditHandler { tools: &h.tools };
    let (first, _) = handler
        .handle(edit_input(&document, &op_id, op.clone()), &ctx, &exec())
        .await
        .expect("first");
    assert!(!first.replayed);
    let (again, summary) = handler
        .handle(edit_input(&document, &op_id, op), &ctx, &exec())
        .await
        .expect("replay");
    assert!(again.replayed, "{summary}");
    assert_eq!(again.revision_id, first.revision_id);
    assert_eq!(again.written, first.written);

    let different = EditOp::StrReplace {
        old_str: "never applied".to_owned(),
        new_str: "x".to_owned(),
    };
    let refused = handler
        .handle(edit_input(&document, &op_id, different), &ctx, &exec())
        .await
        .expect_err("a different request under the same op id");
    let code = refused
        .data
        .as_ref()
        .and_then(|data| data.get("code"))
        .and_then(|code| code.as_str());
    assert_eq!(code, Some("op_id_mismatch"));
}

#[tokio::test]
async fn search_and_meta_views_answer_without_a_full_read() {
    let h = harness().await;
    let owner = registered("views").await;
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "Revenue grew\ncosts fell").await;
    let mut input = read_input(&document, ReadView::Search);
    input.text = Some("REVENUE".to_owned());
    let found = read(&h.tools, &ctx, input).await;
    assert_eq!(found.lines.len(), 1, "{:?}", found.lines);
    assert!(found.lines[0].ends_with("| Revenue grew"));

    let meta = read(&h.tools, &ctx, read_input(&document, ReadView::Meta)).await;
    assert_eq!(meta.revision_id, revision);
    assert!(meta.lines.is_empty());
    let meta = meta.meta.expect("meta");
    assert!(meta.name.starts_with("notes-"));
    assert!(meta.size_bytes.is_some_and(|size| size > 0));
}

#[tokio::test]
async fn a_verbose_read_lists_codec_paths_and_history_shows_attribution() {
    let h = harness().await;
    let owner = registered("attribution").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "first\nsecond").await;
    let mut input = read_input(&document, ReadView::Text);
    input.verbose = true;
    let verbose = read(&h.tools, &ctx, input).await;
    assert!(
        verbose.paths.iter().any(|line| line.ends_with(" line[1]")),
        "{:?}",
        verbose.paths
    );
    let (history, _) = HistoryHandler { tools: &h.tools }
        .handle(
            HistoryInput {
                document_id: document,
                limit: Some(1),
                verbose: true,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("history");
    let attribution = history.revisions[0]
        .attribution
        .as_ref()
        .expect("attribution under verbose");
    assert_eq!(attribution.session_id.as_ref(), Some(ctx.session_id()));
    assert_eq!(attribution.context_id.as_ref(), Some(ctx.context_id()));
    assert_eq!(attribution.trace_id.as_ref(), Some(ctx.trace_id()));
    assert_eq!(attribution.on_behalf_of.as_ref(), Some(&owner.id));
}

fn block(id: &str, kind: NodeKind, text: &str) -> ProjectionNode {
    ProjectionNode {
        id: DocumentNodeId::new(id),
        kind,
        text: Some(text.to_owned()),
        hash8: hash8(text),
        children: Vec::new(),
        attrs: BTreeMap::new(),
    }
}

fn projection(root: Vec<ProjectionNode>) -> Projection {
    Projection {
        format: DocumentFormat::Docx,
        root,
        content_sha256: content_hash(b""),
    }
}

#[test]
fn the_outline_folds_long_lists() {
    let mut nodes = vec![block("a.1", NodeKind::Heading { level: 1 }, "Plan")];
    nodes.extend((0..6).map(|n| block(&format!("a.{}", n + 2), NodeKind::ListItem, "item")));
    nodes.push(block("a.9", NodeKind::Heading { level: 2 }, "Risks"));
    let lines = outline(&projection(nodes));
    assert_eq!(lines.len(), 6, "{lines:?}");
    assert!(lines[0].ends_with("| # Plan"));
    assert!(lines[1..4].iter().all(|line| line.ends_with("| - item")));
    assert_eq!(lines[4], "gap blocks=3");
    assert!(lines[5].ends_with("| ## Risks"));
}

#[test]
fn a_node_read_pages_one_node_and_its_children() {
    let mut table = block("a.2", NodeKind::Table, "a b");
    table.children = vec![
        block("a.3", NodeKind::Cell, "a"),
        block("a.4", NodeKind::Cell, "b"),
    ];
    let doc = projection(vec![
        block("a.1", NodeKind::Paragraph, "before"),
        table,
        block("a.5", NodeKind::Paragraph, "after"),
    ]);
    let lines = node_lines(&doc, Some(&DocumentNodeId::new("a.2"))).expect("node");
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[1].ends_with("| | a"));
    assert!(node_lines(&doc, Some(&DocumentNodeId::new("a.9"))).is_err());
    assert!(node_lines(&doc, None).is_err());
    let found = search_lines(&doc, Some("AFT")).expect("search");
    assert_eq!(found.len(), 1);
}
