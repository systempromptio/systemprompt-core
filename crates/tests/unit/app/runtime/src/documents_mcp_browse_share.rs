use systemprompt_files::documents::service::NewFolderRequest;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::{AccessLevel, DocumentFormat};
use systemprompt_runtime::documents::mcp::{
    ListHandler, ListInput, ListScopeInput, ListSortInput, SearchHandler, SearchInput,
    ShareHandler, ShareInput,
};
use uuid::Uuid;

use crate::documents_mcp_support::{create_text, ctx_for, exec, harness, person, registered};

fn share() -> ShareInput {
    ShareInput {
        document_id: None,
        folder_id: None,
        subject_kind: None,
        subject_value: None,
        level: None,
        valid_until: None,
        transfer_to: None,
        revoke_all: false,
        message: None,
    }
}

fn list(scope: ListScopeInput, sort: ListSortInput) -> ListInput {
    ListInput {
        scope,
        sort,
        folder_id: None,
        query: None,
        cursor: None,
        limit: Some(200),
        verbose: true,
    }
}

fn code(error: &systemprompt_mcp::McpError) -> String {
    error.data.as_ref().expect("refusal data")["code"]
        .as_str()
        .expect("code")
        .to_owned()
}

#[tokio::test]
async fn share_needs_exactly_one_target_and_a_full_subject() {
    let h = harness().await;
    let owner = person("share-shape");
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "x").await;
    let handler = ShareHandler { tools: &h.tools };

    let error = handler
        .handle(share(), &ctx, &exec())
        .await
        .expect_err("no target");
    assert!(
        error
            .message
            .contains("exactly one of document_id or folder_id")
    );

    let partial = ShareInput {
        document_id: Some(document.clone()),
        subject_kind: Some("user".to_owned()),
        ..share()
    };
    let error = handler
        .handle(partial, &ctx, &exec())
        .await
        .expect_err("no level");
    assert_eq!(code(&error), "invalid_params");
    assert!(
        error
            .message
            .contains("subject_kind, subject_value and level")
    );

    let orphan_revoke = ShareInput {
        revoke_all: true,
        ..share()
    };
    let error = handler
        .handle(orphan_revoke, &ctx, &exec())
        .await
        .expect_err("revoke without document");
    assert_eq!(code(&error), "invalid_params");
    assert!(error.message.contains("take a document_id"));

    let orphan_transfer = ShareInput {
        transfer_to: Some(owner.id.clone()),
        ..share()
    };
    let error = handler
        .handle(orphan_transfer, &ctx, &exec())
        .await
        .expect_err("transfer without document");
    assert_eq!(code(&error), "invalid_params");
}

#[tokio::test]
async fn folder_grants_and_revoke_all_on_a_document() {
    let h = harness().await;
    let owner = person("share-folder");
    let reader = registered("share-reader").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "x").await;
    let folder = h
        .tools
        .service()
        .create_folder(
            &owner,
            NewFolderRequest {
                parent: None,
                name: format!("f-{}", Uuid::new_v4().simple()),
            },
            ctx.trace_id(),
        )
        .await
        .expect("folder");
    let handler = ShareHandler { tools: &h.tools };

    let (granted, summary) = handler
        .handle(
            ShareInput {
                folder_id: Some(folder.id.clone()),
                subject_kind: Some("user".to_owned()),
                subject_value: Some(reader.id.as_str().to_owned()),
                level: Some(AccessLevel::View),
                ..share()
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("folder grant");
    assert!(granted.grant.is_some());
    assert!(summary.starts_with("granted view to user"), "{summary}");

    handler
        .handle(
            ShareInput {
                document_id: Some(document.clone()),
                subject_kind: Some("user".to_owned()),
                subject_value: Some(reader.id.as_str().to_owned()),
                level: Some(AccessLevel::Edit),
                ..share()
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("document grant");
    let (revoked, summary) = handler
        .handle(
            ShareInput {
                document_id: Some(document.clone()),
                revoke_all: true,
                ..share()
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("revoke all");
    let count = revoked.revoked.expect("revoked count");
    assert!(count >= 1);
    assert_eq!(summary, format!("revoked {count} grants"));
    assert!(revoked.grant.is_none() && revoked.transferred_to.is_none());

    let (listed, summary) = ListHandler { tools: &h.tools }
        .handle(
            list(ListScopeInput::Folder, ListSortInput::Name),
            &ctx,
            &exec(),
        )
        .await
        .expect("list root");
    let item = listed
        .folders
        .iter()
        .find(|f| f.folder_id == folder.id)
        .expect("folder listed");
    assert_eq!(item.name, folder.name);
    assert!(item.parent_id.is_none());
    let doc = listed
        .items
        .iter()
        .find(|d| d.document_id == document)
        .expect("document listed");
    assert!(doc.uri.is_some());
    assert!(summary.contains("folders and"), "{summary}");
}

#[tokio::test]
async fn list_rejects_a_forged_cursor_and_accepts_every_sort() {
    let h = harness().await;
    let owner = person("cursor");
    let ctx = ctx_for(Some(&owner));
    let error = ListHandler { tools: &h.tools }
        .handle(
            ListInput {
                cursor: Some("not-a-cursor".to_owned()),
                ..list(ListScopeInput::Folder, ListSortInput::Updated)
            },
            &ctx,
            &exec(),
        )
        .await
        .expect_err("forged cursor");
    assert!(
        error.message.contains("\"not-a-cursor\" is not valid"),
        "{}",
        error.message
    );

    let (document, _) = create_text(&h.tools, &ctx, "x").await;
    for (scope, sort) in [
        (ListScopeInput::Mine, ListSortInput::Recent),
        (ListScopeInput::Recent, ListSortInput::Updated),
    ] {
        let (listed, _) = ListHandler { tools: &h.tools }
            .handle(list(scope, sort), &ctx, &exec())
            .await
            .expect("list");
        assert!(listed.items.iter().any(|d| d.document_id == document));
    }
    let (shared, _) = ListHandler { tools: &h.tools }
        .handle(
            list(ListScopeInput::Shared, ListSortInput::Name),
            &ctx,
            &exec(),
        )
        .await
        .expect("shared");
    assert!(shared.items.iter().all(|d| d.document_id != document));
}

#[tokio::test]
async fn search_finds_the_callers_text_and_filters_by_format() {
    let h = harness().await;
    let owner = person("search");
    let ctx = ctx_for(Some(&owner));
    let word = format!("zq{}", Uuid::new_v4().simple());
    let (document, revision) = create_text(&h.tools, &ctx, &format!("alpha {word} omega")).await;
    let search = |format| SearchInput {
        query: word.clone(),
        format,
        limit: None,
    };
    let handler = SearchHandler { tools: &h.tools };
    let (found, summary) = handler
        .handle(search(None), &ctx, &exec())
        .await
        .expect("search");
    assert_eq!(found.hits.len(), 1, "{summary}");
    assert_eq!(found.hits[0].document_id, document);
    assert_eq!(found.hits[0].revision_id, revision);
    assert_eq!(summary, "1 matching documents");

    let (filtered, _) = handler
        .handle(search(Some(DocumentFormat::Docx)), &ctx, &exec())
        .await
        .expect("filtered");
    assert!(filtered.hits.is_empty());

    let (stranger, _) = handler
        .handle(search(None), &ctx_for(Some(&person("other"))), &exec())
        .await
        .expect("stranger search");
    assert!(stranger.hits.is_empty());
}
