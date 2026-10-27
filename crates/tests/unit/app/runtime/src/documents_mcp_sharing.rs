use systemprompt_identifiers::{DocumentId, UserId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::AccessLevel;
use systemprompt_runtime::documents::mcp::{
    AccessHandler, DocumentInput, ReadHandler, ReadInput, ShareHandler, ShareInput, TrashHandler,
    refusal_result,
};

use crate::documents_mcp_support::{create_text, ctx_for, exec, harness, person};

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
    let owner = person("mcp-own");
    let sharer = person("mcp-sharer");
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
    let owner = person("mcp-transfer-unknown");
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
    let owner = person("mcp-probe-owner");
    let stranger = person("mcp-probe-stranger");
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
    let owner = person("mcp-access-owner");
    let viewer = person("mcp-access-viewer");
    let other = person("mcp-access-other");
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
            DocumentInput {
                document_id: document.clone(),
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
            DocumentInput {
                document_id: document,
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("owner reads access");
    assert_eq!(full.grants.len(), 2);
}
