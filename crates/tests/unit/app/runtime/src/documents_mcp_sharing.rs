use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::AccessLevel;
use systemprompt_runtime::documents::mcp::{
    AccessHandler, AccessInput, DocumentInput, ReadHandler, ReadInput, ShareHandler, ShareInput,
    TrashHandler, refusal_result,
};

use crate::documents_mcp_support::{create_text, ctx_for, exec, harness, registered};

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
        message: None,
    }
}

fn grant(document: &DocumentId, user: &UserId, level: AccessLevel) -> ShareInput {
    ShareInput {
        subject_kind: Some("user".to_owned()),
        subject_value: Some(user.as_str().to_owned()),
        level: Some(level),
        ..share(document)
    }
}

fn refusal_of(error: &rmcp::ErrorData) -> serde_json::Value {
    refusal_result(error)
        .and_then(|result| result.structured_content)
        .expect("structured refusal")
}

#[tokio::test]
async fn own_is_not_grantable_and_a_share_holder_cannot_trash_or_revoke_all() {
    let h = harness().await;
    let owner = registered("mcp-own").await;
    let sharer = registered("mcp-sharer").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let tool = ShareHandler { tools: &h.tools };
    let refused = tool
        .handle(
            grant(&document, &sharer.id, AccessLevel::Own),
            &ctx,
            &exec(),
        )
        .await
        .expect_err("own refused");
    assert_eq!(refusal_of(&refused)["code"], "invalid_params");
    tool.handle(
        grant(&document, &sharer.id, AccessLevel::Share),
        &ctx,
        &exec(),
    )
    .await
    .expect("owner shares");
    let sharer_ctx = ctx_for(Some(&sharer));
    let trashed = TrashHandler { tools: &h.tools }
        .handle(
            DocumentInput {
                document_id: document.clone(),
            },
            &sharer_ctx,
            &exec(),
        )
        .await
        .expect_err("a share holder cannot trash");
    assert_eq!(refusal_of(&trashed)["code"], "forbidden");
    let revoked = tool
        .handle(
            ShareInput {
                revoke_all: true,
                ..share(&document)
            },
            &sharer_ctx,
            &exec(),
        )
        .await
        .expect_err("a share holder cannot revoke every grant");
    assert_eq!(refusal_of(&revoked)["code"], "forbidden");
}

#[tokio::test]
async fn transfer_to_an_unknown_user_is_refused() {
    let h = harness().await;
    let owner = registered("mcp-transfer-unknown").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let refused = ShareHandler { tools: &h.tools }
        .handle(
            ShareInput {
                transfer_to: Some(UserId::new("nobody-at-all")),
                ..share(&document)
            },
            &ctx,
            &exec(),
        )
        .await
        .expect_err("unknown recipient");
    assert_eq!(refusal_of(&refused)["code"], "unknown_subject");
}

#[tokio::test]
async fn a_non_member_probe_is_not_found_without_the_owner() {
    let h = harness().await;
    let owner = registered("mcp-probe-owner").await;
    let stranger = registered("mcp-probe-stranger").await;
    let (document, _) = create_text(&h.tools, &ctx_for(Some(&owner)), "secret\n").await;
    let refused = ReadHandler { tools: &h.tools }
        .handle(
            serde_json::from_value::<ReadInput>(serde_json::json!({
                "document_id": document.as_str(),
            }))
            .expect("read input"),
            &ctx_for(Some(&stranger)),
            &exec(),
        )
        .await
        .expect_err("stranger refused");
    let data = refusal_of(&refused);
    assert_eq!(data["code"], "not_found");
    assert!(data.get("owner").is_none(), "{data}");
    assert!(!refused.message.contains(owner.id.as_str()));
}

#[tokio::test]
async fn a_viewer_reads_only_the_owner_and_their_own_level() {
    let h = harness().await;
    let owner = registered("mcp-access-owner").await;
    let viewer = registered("mcp-access-viewer").await;
    let other = registered("mcp-access-other").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let tool = ShareHandler { tools: &h.tools };
    tool.handle(
        grant(&document, &viewer.id, AccessLevel::View),
        &ctx,
        &exec(),
    )
    .await
    .expect("share viewer");
    tool.handle(
        grant(&document, &other.id, AccessLevel::Edit),
        &ctx,
        &exec(),
    )
    .await
    .expect("share other");
    let (access, _) = AccessHandler { tools: &h.tools }
        .handle(
            AccessInput {
                document_id: document.clone(),
                history: false,
            },
            &ctx_for(Some(&viewer)),
            &exec(),
        )
        .await
        .expect("viewer reads access");
    assert_eq!(access.owner, owner.id);
    assert_eq!(access.my_access, AccessLevel::View);
    assert!(access.grants.is_empty() && access.inherited.is_empty());
    let (full, _) = AccessHandler { tools: &h.tools }
        .handle(
            AccessInput {
                document_id: document,
                history: false,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("owner reads access");
    assert_eq!(full.grants.len(), 2);
}

#[tokio::test]
async fn unknown_users_and_roles_are_refused_with_unknown_subject() {
    let h = harness().await;
    let owner = registered("mcp-unknown-subject").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let tool = ShareHandler { tools: &h.tools };
    for (kind, value) in [("user", "nobody-at-all"), ("role", "no-such-role-anywhere")] {
        let refused = tool
            .handle(
                ShareInput {
                    subject_kind: Some(kind.to_owned()),
                    subject_value: Some(value.to_owned()),
                    level: Some(AccessLevel::View),
                    ..share(&document)
                },
                &ctx,
                &exec(),
            )
            .await
            .expect_err("unknown subject");
        assert_eq!(refusal_of(&refused)["code"], "unknown_subject", "{kind}");
    }
    tool.handle(
        ShareInput {
            subject_kind: Some("role".to_owned()),
            subject_value: Some("user".to_owned()),
            level: Some(AccessLevel::View),
            ..share(&document)
        },
        &ctx,
        &exec(),
    )
    .await
    .expect("a catalogue role is accepted");
}

#[tokio::test]
async fn access_history_is_returned_on_request_to_share_holders_only() {
    let h = harness().await;
    let owner = registered("mcp-history-owner").await;
    let viewer = registered("mcp-history-viewer").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "body\n").await;
    let mut granted = grant(&document, &viewer.id, AccessLevel::View);
    granted.message = Some("for the Friday review".to_owned());
    ShareHandler { tools: &h.tools }
        .handle(granted, &ctx, &exec())
        .await
        .expect("share");
    let access = |who| {
        let ctx = ctx_for(Some(who));
        let document = document.clone();
        let tools = &h.tools;
        async move {
            AccessHandler { tools }
                .handle(
                    AccessInput {
                        document_id: document,
                        history: true,
                    },
                    &ctx,
                    &exec(),
                )
                .await
                .expect("access")
                .0
        }
    };
    let owned = access(&owner).await;
    assert_eq!(owned.history.len(), 1);
    assert_eq!(
        serde_json::to_value(&owned.history[0]).expect("json")["event"],
        "granted"
    );
    let viewed = access(&viewer).await;
    assert!(viewed.history.is_empty() && viewed.grants.is_empty());
}
