use systemprompt_identifiers::DocumentNodeId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{AccessLevel, ActorKind, ContentAs, EditOp, NodeTarget};
use systemprompt_runtime::documents::mcp::{
    EditHandler, EditInput, ExportAs, ExportFormat, ExportHandler, ExportInput, HistoryHandler,
    HistoryInput, ListHandler, ListInput, ListScopeInput, ListSortInput, ReadHandler, ReadInput,
    ReadView, ShareHandler, ShareInput,
};

use crate::documents_mcp_support::{
    create_text, ctx_for, exec, harness, line_for, person, read_text,
};

#[tokio::test]
async fn read_returns_hashlines_and_terse_ids() {
    let h = harness().await;
    let owner = person("reader");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "alpha\nbeta").await;
    let read = read_text(&h.tools, &ctx, &document).await;
    assert_eq!(read.revision_id, revision);
    assert!(read.uri.is_none());
    assert!(read.content_sha256.is_none());
    let (node, hash) = line_for(&read, "beta");
    assert!(!node.starts_with("line["), "node {node}");
    assert!(node.contains('.'), "node {node}");
    assert_eq!(hash, systemprompt_models::documents::hash8("beta"));
}

#[tokio::test]
async fn stale_anchor_names_node_hashes_and_current_text() {
    let h = harness().await;
    let owner = person("anchor");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "alpha\nbeta").await;
    let read = read_text(&h.tools, &ctx, &document).await;
    let (node, actual) = line_for(&read, "beta");
    let error = EditHandler { tools: &h.tools }
        .handle(
            EditInput {
                document_id: document,
                base_revision: Some(revision),
                op_id: None,
                answering: None,
                allow_detach: false,
                ops: vec![EditOp::ReplaceText {
                    node: NodeTarget::Id(DocumentNodeId::new(node)),
                    expected_hash: Some("deadbeef".to_owned()),
                    text: "gamma".to_owned(),
                    content_as: ContentAs::Text,
                }],
                message: None,
                verbose: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect_err("stale anchor");
    for needle in [node, "deadbeef", actual, "\"beta\""] {
        assert!(
            error.message.contains(needle),
            "missing {needle}: {}",
            error.message
        );
    }
}

#[tokio::test]
async fn edit_acknowledges_the_written_lines_and_records_the_message() {
    let h = harness().await;
    let owner = person("editor");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "alpha\nbeta").await;
    let read = read_text(&h.tools, &ctx, &document).await;
    let (node, hash) = line_for(&read, "beta");
    let (edited, _) = EditHandler { tools: &h.tools }
        .handle(
            EditInput {
                document_id: document.clone(),
                base_revision: Some(revision.clone()),
                op_id: None,
                answering: None,
                allow_detach: false,
                ops: vec![EditOp::ReplaceText {
                    node: NodeTarget::Id(DocumentNodeId::new(node)),
                    expected_hash: Some(hash.to_owned()),
                    text: "gamma".to_owned(),
                    content_as: ContentAs::Text,
                }],
                message: Some("rename".to_owned()),
                verbose: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("edit");
    assert!(edited.parent_revision_id.is_none());
    assert_eq!(edited.applied, 1);
    assert_eq!(
        edited.written,
        vec![format!(
            "{node}:{}| gamma",
            systemprompt_models::documents::hash8("gamma")
        )]
    );
    let (history, _) = HistoryHandler { tools: &h.tools }
        .handle(
            HistoryInput {
                document_id: document,
                limit: None,
                verbose: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("history");
    let head = &history.revisions[0];
    assert_eq!(head.revision_id, edited.revision_id);
    assert_eq!(head.actor_kind, ActorKind::Agent);
    assert_eq!(head.author_user_id, owner.id);
    assert_eq!(head.message.as_deref(), Some("rename"));
    assert!(head.uri.is_none() && head.content_sha256.is_none());
}

#[tokio::test]
async fn str_replace_reports_the_match_count() {
    let h = harness().await;
    let owner = person("replace");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "alpha\nbeta\nalpha").await;
    let replace = |old: &str| EditInput {
        document_id: document.clone(),
        base_revision: Some(revision.clone()),
        op_id: None,
        answering: None,
        allow_detach: false,
        ops: vec![EditOp::StrReplace {
            old_str: old.to_owned(),
            new_str: "omega".to_owned(),
        }],
        message: Some("swap".to_owned()),
        verbose: false,
    };
    let handler = EditHandler { tools: &h.tools };
    let twice = handler
        .handle(replace("alpha"), &ctx, &exec())
        .await
        .expect_err("ambiguous");
    assert!(
        twice.message.contains("matches 2 places"),
        "{}",
        twice.message
    );
    let none = handler
        .handle(replace("zeta"), &ctx, &exec())
        .await
        .expect_err("no match");
    assert!(
        none.message.contains("matches 0 places"),
        "{}",
        none.message
    );
    let (once, _) = handler
        .handle(replace("beta"), &ctx, &exec())
        .await
        .expect("unique match");
    assert!(
        once.written.iter().any(|line| line.ends_with("| omega")),
        "{:?}",
        once.written
    );
}

#[tokio::test]
async fn stale_base_names_the_head_revision() {
    let h = harness().await;
    let owner = person("stale");
    let ctx = ctx_for(Some(&owner));
    let (document, base) = create_text(&h.tools, &ctx, "alpha\nbeta").await;
    let set = |text: &str| EditInput {
        document_id: document.clone(),
        base_revision: Some(base.clone()),
        op_id: None,
        answering: None,
        allow_detach: false,
        ops: vec![EditOp::SetText {
            text: text.to_owned(),
        }],
        message: None,
        verbose: false,
    };
    let handler = EditHandler { tools: &h.tools };
    let (first, _) = handler
        .handle(set("alpha\nfirst"), &ctx, &exec())
        .await
        .expect("first edit");
    let error = handler
        .handle(set("alpha\nsecond"), &ctx, &exec())
        .await
        .expect_err("conflicting edit on a stale base");
    assert!(
        error.message.contains(first.revision_id.as_str()),
        "{}",
        error.message
    );
    assert!(error.message.contains("base_revision"), "{}", error.message);
    let data = error.data.as_ref().expect("refusal data");
    assert_eq!(data["code"], "stale_base");
    assert_eq!(data["head_revision"], first.revision_id.as_str());
    assert_eq!(data["written_by"], owner.id.as_str());
    assert_eq!(data["actor_kind"], "agent");
}

#[tokio::test]
async fn forbidden_names_the_needed_and_held_level() {
    let h = harness().await;
    let owner = person("owner");
    let stranger = person("stranger");
    let (document, _) = create_text(&h.tools, &ctx_for(Some(&owner)), "secret").await;
    let read = |document| ReadInput {
        document_id: document,
        view: ReadView::Text,
        range: None,
        sheet: None,
        cells: None,
        revision_id: None,
        since_revision: None,
        node_id: None,
        text: None,
        verbose: false,
    };
    let error = ReadHandler { tools: &h.tools }
        .handle(read(document.clone()), &ctx_for(Some(&stranger)), &exec())
        .await
        .expect_err("stranger cannot read");
    assert!(error.message.contains("forbidden"), "{}", error.message);
    assert!(error.message.contains("view access"), "{}", error.message);
    assert!(error.message.contains("no access"), "{}", error.message);

    ShareHandler { tools: &h.tools }
        .handle(
            ShareInput {
                document_id: Some(document.clone()),
                folder_id: None,
                subject_kind: Some("user".to_owned()),
                subject_value: Some(stranger.id.as_str().to_owned()),
                level: Some(AccessLevel::View),
                valid_until: None,
                transfer_to: None,
                revoke_all: false,
            },
            &ctx_for(Some(&owner)),
            &exec(),
        )
        .await
        .expect("share");
    ReadHandler { tools: &h.tools }
        .handle(read(document), &ctx_for(Some(&stranger)), &exec())
        .await
        .expect("shared reader can read");
}

#[tokio::test]
async fn a_call_without_an_authenticated_user_is_refused() {
    let h = harness().await;
    let error = ListHandler { tools: &h.tools }
        .handle(
            ListInput {
                scope: ListScopeInput::Folder,
                sort: ListSortInput::Updated,
                folder_id: None,
                query: None,
                cursor: None,
                limit: None,
                verbose: false,
            },
            &ctx_for(None),
            &exec(),
        )
        .await
        .expect_err("anonymous");
    assert!(
        error.message.contains("authenticated user"),
        "{}",
        error.message
    );
}

#[tokio::test]
async fn list_shows_the_owners_root_document() {
    let h = harness().await;
    let owner = person("lister");
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "x").await;
    let (listed, _) = ListHandler { tools: &h.tools }
        .handle(
            ListInput {
                scope: ListScopeInput::Folder,
                sort: ListSortInput::Updated,
                folder_id: None,
                query: Some("notes-".to_owned()),
                cursor: None,
                limit: Some(200),
                verbose: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("list");
    assert!(listed.items.iter().any(|item| item.document_id == document));
}

#[tokio::test]
async fn export_links_and_previews_the_revision() {
    let h = harness().await;
    let owner = person("download");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "hello").await;
    let input = |format, export_as| ExportInput {
        document_id: document.clone(),
        revision_id: None,
        format,
        export_as,
    };
    let (download, _) = ExportHandler { tools: &h.tools }
        .handle(input(ExportFormat::Native, ExportAs::Url), &ctx, &exec())
        .await
        .expect("download url");
    assert_eq!(download.revision_id, revision);
    let url = download.url.clone().expect("url");
    assert!(
        url.starts_with(&format!(
            "https://docs.example.test/api/v1/documents/{document}/content?revision={revision}&access_token="
        )),
        "{url}"
    );
    let lifetime = download.expires_at.expect("expiry") - chrono::Utc::now();
    assert!(lifetime <= chrono::Duration::minutes(10) && lifetime > chrono::Duration::minutes(9));

    let (preview, _) = ExportHandler { tools: &h.tools }
        .handle(input(ExportFormat::Html, ExportAs::Preview), &ctx, &exec())
        .await
        .expect("preview");
    assert_eq!(
        preview.ui_resource.as_deref(),
        Some("ui://documents/document-preview")
    );
    assert!(
        preview
            .html
            .as_deref()
            .is_some_and(|html| html.contains("hello"))
    );
    let pdf = ExportHandler { tools: &h.tools }
        .handle(input(ExportFormat::Pdf, ExportAs::Url), &ctx, &exec())
        .await
        .expect_err("no codec renders pdf");
    assert_eq!(
        pdf.data.as_ref().expect("refusal")["code"],
        "unsupported_format"
    );
}

#[tokio::test]
async fn create_applies_ops_in_the_first_revision_and_acknowledges_them() {
    use systemprompt_identifiers::DocumentTemplateId;
    use systemprompt_models::documents::NodeKind;
    use systemprompt_runtime::documents::mcp::{CreateHandler, CreateInput};
    let h = harness().await;
    let owner = person("creator");
    let ctx = ctx_for(Some(&owner));
    let (created, _) = CreateHandler { tools: &h.tools }
        .handle(
            CreateInput {
                template_id: DocumentTemplateId::new("blank-html"),
                folder_id: None,
                name: format!("memo-{}", uuid::Uuid::new_v4().simple()),
                params: Default::default(),
                ops: vec![
                    EditOp::InsertAfter {
                        node: NodeTarget::Id(DocumentNodeId::new("body/h1[1]")),
                        kind: NodeKind::Paragraph,
                        text: "first".to_owned(),
                        content_as: ContentAs::Text,
                    },
                    EditOp::InsertAfter {
                        node: NodeTarget::Id(DocumentNodeId::new("body/h1[1]")),
                        kind: NodeKind::Paragraph,
                        text: "second".to_owned(),
                        content_as: ContentAs::Text,
                    },
                ],
                message: Some("draft".to_owned()),
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("create with ops");
    let shown: Vec<&str> = created
        .written
        .iter()
        .filter_map(|line| line.split_once("| ").map(|(_, text)| text))
        .collect();
    let first = shown.iter().position(|t| *t == "first").expect("first");
    assert_eq!(shown.get(first + 1), Some(&"second"), "{shown:?}");
    let (history, _) = HistoryHandler { tools: &h.tools }
        .handle(
            HistoryInput {
                document_id: created.document_id,
                limit: None,
                verbose: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("history");
    assert_eq!(history.revisions.len(), 1);
    assert_eq!(history.revisions[0].message.as_deref(), Some("draft"));
}
