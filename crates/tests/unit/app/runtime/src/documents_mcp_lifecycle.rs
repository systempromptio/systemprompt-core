use systemprompt_files::documents::service::FolderMove;
use systemprompt_identifiers::{DocumentId, DocumentNodeId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{AccessLevel, ContentAs, EditOp, NodeTarget};
use systemprompt_runtime::documents::mcp::{
    AccessHandler, CommentAction, CommentHandler, CommentInput, CopyHandler, CopyInput,
    DocumentInput, EditHandler, EditInput, GuideHandler, GuideInput, ListHandler, ListInput,
    ListScopeInput, ListSortInput, QuoteAnchor, RestoreHandler, ShareHandler, ShareInput,
    TrashHandler, UpdateHandler, UpdateInput, refusal_result,
};

use crate::documents_mcp_support::{
    create_text, ctx_for, exec, harness, line_for, person, read_text,
};

fn comment(action: CommentAction, document: &DocumentId) -> CommentInput {
    CommentInput {
        action,
        document_id: document.clone(),
        node: None,
        anchor: None,
        body: None,
        parent_id: None,
        comment_id: None,
        answering: None,
    }
}

fn share(document: &DocumentId) -> ShareInput {
    ShareInput {
        document_id: Some(document.clone()),
        folder_id: None,
        subject_kind: None,
        subject_value: None,
        level: None,
        valid_until: None,
        transfer_to: None,
        revoke_all: false,
    }
}

#[tokio::test]
async fn comment_tool_creates_lists_and_resolves_and_edits_refuse_anchors_affected() {
    let h = harness().await;
    let owner = person("mcp-comment");
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "alpha beta\nsecond\n").await;
    let read = read_text(&h.tools, &ctx, &document).await;
    let (node, hash) = line_for(&read, "alpha beta");
    let (node, hash) = (node.to_owned(), hash.to_owned());

    let (created, _) = CommentHandler { tools: &h.tools }
        .handle(
            CommentInput {
                node: Some(DocumentNodeId::new(node.clone())),
                anchor: Some(QuoteAnchor {
                    find: "beta".to_owned(),
                    nth: None,
                }),
                body: Some("why beta?".to_owned()),
                ..comment(CommentAction::Create, &document)
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("create comment");
    let id = created.comments[0].id.clone();

    let edit = |allow_detach: bool| EditInput {
        document_id: document.clone(),
        base_revision: None,
        mode: None,
        op_id: None,
        answering: None,
        allow_detach,
        ops: vec![EditOp::ReplaceText {
            node: NodeTarget::Id(DocumentNodeId::new(node.clone())),
            expected_hash: Some(hash.clone()),
            text: "alpha gamma".to_owned(),
            content_as: ContentAs::Text,
        }],
        message: None,
        verbose: false,
    };
    let refused = EditHandler { tools: &h.tools }
        .handle(edit(false), &ctx, &exec())
        .await
        .expect_err("anchored words removed");
    let result = refusal_result(&refused).expect("refusal");
    let data = result.structured_content.expect("structured");
    assert_eq!(data["code"], "anchors_affected");
    assert_eq!(data["comments"][0], id.as_str());
    EditHandler { tools: &h.tools }
        .handle(edit(true), &ctx, &exec())
        .await
        .expect("detach allowed");

    let (resolved, _) = CommentHandler { tools: &h.tools }
        .handle(
            CommentInput {
                comment_id: Some(id.clone()),
                ..comment(CommentAction::Resolve, &document)
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("resolve");
    assert!(resolved.comments[0].resolved_at.is_some());
    let (listed, _) = CommentHandler { tools: &h.tools }
        .handle(comment(CommentAction::List, &document), &ctx, &exec())
        .await
        .expect("list");
    assert_eq!(listed.comments.len(), 1);
    assert!(listed.comments[0].anchor.find.is_none());
}

#[tokio::test]
async fn trash_restore_copy_update_and_access_tools() {
    let h = harness().await;
    let owner = person("mcp-life");
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let input = || DocumentInput {
        document_id: document.clone(),
    };

    let (trashed, _) = TrashHandler { tools: &h.tools }
        .handle(input(), &ctx, &exec())
        .await
        .expect("trash");
    assert!(trashed.trashed);
    let refused = AccessHandler { tools: &h.tools }
        .handle(input(), &ctx, &exec())
        .await
        .expect_err("trashed");
    let data = refusal_result(&refused)
        .and_then(|r| r.structured_content)
        .expect("refusal");
    assert_eq!(data["code"], "not_found");
    assert_eq!(data["trashed"], true);
    RestoreHandler { tools: &h.tools }
        .handle(input(), &ctx, &exec())
        .await
        .expect("restore");

    let fresh = format!("renamed-{}", uuid::Uuid::new_v4().simple());
    let (updated, _) = UpdateHandler { tools: &h.tools }
        .handle(
            UpdateInput {
                document_id: document.clone(),
                name: Some(fresh.clone()),
                folder_id: FolderMove::Keep,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("update");
    assert_eq!(updated.document.name, format!("{fresh}.txt"));
    let (copied, _) = CopyHandler { tools: &h.tools }
        .handle(
            CopyInput {
                document_id: document.clone(),
                name: Some(format!("duplicate-{}", uuid::Uuid::new_v4().simple())),
                folder_id: None,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("copy");
    assert_ne!(copied.document.document_id, document);

    let (access, _) = AccessHandler { tools: &h.tools }
        .handle(input(), &ctx, &exec())
        .await
        .expect("access");
    assert_eq!(access.owner, owner.id);
    assert_eq!(access.my_access, AccessLevel::Own);
}

#[tokio::test]
async fn share_transfers_ownership_and_lists_show_access() {
    let h = harness().await;
    let owner = person("mcp-transfer");
    let next = person("mcp-next");
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let (out, _) = ShareHandler { tools: &h.tools }
        .handle(
            ShareInput {
                transfer_to: Some(next.id.clone()),
                ..share(&document)
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("transfer");
    assert_eq!(out.transferred_to.as_ref(), Some(&next.id));

    let (listed, _) = ListHandler { tools: &h.tools }
        .handle(
            ListInput {
                scope: ListScopeInput::Shared,
                folder_id: None,
                query: None,
                sort: ListSortInput::Updated,
                cursor: None,
                limit: Some(200),
                verbose: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("list shared");
    let item = listed
        .items
        .iter()
        .find(|item| item.document_id == document)
        .expect("the previous owner sees it as shared");
    assert_eq!(item.my_access, Some(AccessLevel::Edit));
    assert!(item.size_bytes.is_some());
}

#[tokio::test]
async fn the_guide_explains_comments_lifecycle_and_skeletons() {
    let h = harness().await;
    let owner = person("mcp-guide-new");
    let (_, text) = GuideHandler { tools: &h.tools }
        .handle(
            GuideInput {
                topics: vec![
                    "comments".to_owned(),
                    "lifecycle".to_owned(),
                    "editing".to_owned(),
                ],
            },
            &ctx_for(Some(&owner)),
            &exec(),
        )
        .await
        .expect("guide");
    for needle in ["allow_detach", "documents_restore_document", "pending"] {
        assert!(text.contains(needle), "guide lacks {needle}");
    }
}

#[test]
fn update_folder_id_is_tri_state() {
    let id = DocumentId::generate();
    let parse = |extra: &str| -> UpdateInput {
        serde_json::from_str(&format!("{{\"document_id\":\"{id}\"{extra}}}")).expect("parse")
    };
    assert_eq!(parse("").folder_id, FolderMove::Keep);
    assert_eq!(parse(",\"folder_id\":null").folder_id, FolderMove::Root);
    let folder = systemprompt_identifiers::DocumentFolderId::generate();
    assert_eq!(
        parse(&format!(",\"folder_id\":\"{folder}\"")).folder_id,
        FolderMove::Into(folder)
    );
}

#[tokio::test]
async fn update_moves_a_document_back_to_the_root() {
    let h = harness().await;
    let owner = person("mcp-root");
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let folder = h
        .tools
        .service()
        .create_folder(
            &owner,
            systemprompt_files::documents::service::NewFolderRequest {
                parent: None,
                name: format!("f-{}", uuid::Uuid::new_v4().simple()),
            },
            ctx.trace_id(),
        )
        .await
        .expect("folder")
        .id;
    let update = |folder_id| UpdateInput {
        document_id: document.clone(),
        name: None,
        folder_id,
    };
    let (moved, _) = UpdateHandler { tools: &h.tools }
        .handle(update(FolderMove::Into(folder.clone())), &ctx, &exec())
        .await
        .expect("into folder");
    assert_eq!(moved.document.folder_id, Some(folder));
    let (home, _) = UpdateHandler { tools: &h.tools }
        .handle(update(FolderMove::Root), &ctx, &exec())
        .await
        .expect("to the root");
    assert_eq!(home.document.folder_id, None);
}
