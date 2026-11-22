//! `documents_list` with `scope: "context"`: the calling conversation by
//! default, and a conversation the caller does not own refused as
//! `unknown_context`.

use serde_json::json;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_models::documents::AccessLevel;
use systemprompt_runtime::documents::mcp::{ListHandler, ListInput};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, registered};

#[tokio::test]
async fn a_context_listing_defaults_to_the_calling_conversation() {
    let r = rig(false).await;
    let user = registered("list-context").await;
    let here = ctx_for(Some(&user));
    let there = ctx_for(Some(&user));
    let (written_here, _) = create_text(&r.tools, &here, "alpha").await;
    let (written_there, _) = create_text(&r.tools, &there, "beta").await;

    let input: ListInput = serde_json::from_value(json!({"scope": "context"})).expect("list input");
    let (listing, _) = ListHandler { tools: &r.tools }
        .handle(input, &here, &exec())
        .await
        .expect("list");
    let listed: Vec<_> = listing.items.iter().map(|i| &i.document_id).collect();
    assert_eq!(listed, vec![&written_here], "{listed:?}");
    assert!(!listed.contains(&&written_there));
    assert_eq!(listing.items[0].my_access, Some(AccessLevel::Own));
    assert!(listing.folders.is_empty());

    let named: ListInput = serde_json::from_value(json!({
        "scope": "context",
        "context_id": here.context_id().as_str(),
    }))
    .expect("list input");
    let (again, _) = ListHandler { tools: &r.tools }
        .handle(named, &here, &exec())
        .await
        .expect("named list");
    assert_eq!(again.items.len(), 1);
}

#[tokio::test]
async fn a_conversation_the_caller_does_not_own_is_unknown_context() {
    let r = rig(false).await;
    let user = registered("list-context-foreign").await;
    let here = ctx_for(Some(&user));
    let there = ctx_for(Some(&user));
    create_text(&r.tools, &there, "beta").await;
    let input: ListInput = serde_json::from_value(json!({
        "scope": "context",
        "context_id": there.context_id().as_str(),
    }))
    .expect("list input");
    let error = ListHandler { tools: &r.tools }
        .handle(input, &here, &exec())
        .await
        .expect_err("another session's derived context is not owned");
    assert_eq!(code_of(&error), RefusalCode::UnknownContext);
}
