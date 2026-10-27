use systemprompt_identifiers::DocumentNodeId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{AccessLevel, ActorKind, EditOp};
use systemprompt_runtime::documents::mcp::{
    DownloadUrlHandler, EditHandler, EditInput, HistoryHandler, HistoryInput, ListHandler,
    ListInput, ReadHandler, ReadInput, ReadView, RenderPreviewHandler, RevisionInput, ShareHandler,
    ShareInput, StrReplaceHandler, StrReplaceInput,
};

use crate::documents_mcp_support::{
    create_text, ctx_for, exec, harness, line_for, person, read_text,
};

#[tokio::test]
async fn read_returns_hashlines_and_a_document_uri() {
    let h = harness().await;
    let owner = person("reader");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "alpha\nbeta").await;
    let read = read_text(&h.tools, &ctx, &document).await;
    assert_eq!(read.revision_id, revision);
    assert_eq!(
        read.uri.to_string(),
        format!("document://{document}@{revision}")
    );
    let (node, hash) = line_for(&read, "beta");
    assert!(node.starts_with("line["), "node {node}");
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
                base_revision: revision,
                ops: vec![EditOp::ReplaceText {
                    node: DocumentNodeId::new(node),
                    expected_hash: "deadbeef".to_owned(),
                    text: "gamma".to_owned(),
                }],
                message: None,
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
async fn edit_records_an_agent_revision_with_changed_node_uris() {
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
                base_revision: revision.clone(),
                ops: vec![EditOp::ReplaceText {
                    node: DocumentNodeId::new(node),
                    expected_hash: hash.to_owned(),
                    text: "gamma".to_owned(),
                }],
                message: Some("rename".to_owned()),
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("edit");
    assert_eq!(edited.parent_revision_id.as_ref(), Some(&revision));
    assert_eq!(edited.applied, 1);
    assert_eq!(
        edited.changed_nodes[0].to_string(),
        format!("document://{document}@{}#{node}", edited.revision_id)
    );
    let (history, _) = HistoryHandler { tools: &h.tools }
        .handle(
            HistoryInput {
                document_id: document,
                limit: None,
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
}

#[tokio::test]
async fn str_replace_reports_the_match_count() {
    let h = harness().await;
    let owner = person("replace");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "alpha\nbeta\nalpha").await;
    let replace = |old: &str| StrReplaceInput {
        document_id: document.clone(),
        base_revision: revision.clone(),
        old_str: old.to_owned(),
        new_str: "omega".to_owned(),
    };
    let handler = StrReplaceHandler { tools: &h.tools };
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
    assert!(once.reread_hint.is_some());
}

#[tokio::test]
async fn stale_base_names_the_head_revision() {
    let h = harness().await;
    let owner = person("stale");
    let ctx = ctx_for(Some(&owner));
    let (document, base) = create_text(&h.tools, &ctx, "alpha\nbeta").await;
    let set = |text: &str| EditInput {
        document_id: document.clone(),
        base_revision: base.clone(),
        ops: vec![EditOp::SetText {
            text: text.to_owned(),
        }],
        message: None,
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
                subject_kind: "user".to_owned(),
                subject_value: stranger.id.as_str().to_owned(),
                level: AccessLevel::View,
                valid_until: None,
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
                folder_id: None,
                query: None,
                cursor: None,
                limit: None,
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
                folder_id: None,
                query: Some("notes-".to_owned()),
                cursor: None,
                limit: Some(200),
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("list");
    assert!(listed.items.iter().any(|item| item.document_id == document));
}

#[tokio::test]
async fn download_url_and_preview_address_the_revision() {
    let h = harness().await;
    let owner = person("download");
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "hello").await;
    let input = RevisionInput {
        document_id: document.clone(),
        revision_id: None,
    };
    let (download, _) = DownloadUrlHandler { tools: &h.tools }
        .handle(input.clone(), &ctx, &exec())
        .await
        .expect("download url");
    assert_eq!(download.revision_id, revision);
    assert!(
        download.url.starts_with(&format!(
            "https://docs.example.test/api/v1/documents/{document}/content?revision={revision}&access_token="
        )),
        "{}",
        download.url
    );
    let lifetime = download.expires_at - chrono::Utc::now();
    assert!(lifetime <= chrono::Duration::minutes(10) && lifetime > chrono::Duration::minutes(9));

    let (preview, _) = RenderPreviewHandler { tools: &h.tools }
        .handle(input, &ctx, &exec())
        .await
        .expect("preview");
    assert_eq!(preview.ui_resource, "ui://documents/document-preview");
    assert!(preview.html.contains("hello"), "{}", preview.html);
}
