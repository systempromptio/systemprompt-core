use serde_json::json;
use systemprompt_identifiers::NotificationId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    DocumentsMcpTools, InboxAction, InboxHandler, InboxInput, InboxOutput,
};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{ctx_for, exec, registered};

fn input(value: serde_json::Value) -> InboxInput {
    serde_json::from_value(value).expect("inbox input")
}

async fn call(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<(InboxOutput, String), rmcp::ErrorData> {
    InboxHandler { tools }
        .handle(input(value), ctx, &exec())
        .await
}

#[test]
fn inbox_input_reads_each_action() {
    assert_eq!(input(json!({"action": "list"})).action, InboxAction::List);
    assert_eq!(
        input(json!({"action": "mark_read", "all": true})).action,
        InboxAction::MarkRead
    );
    let id = NotificationId::generate();
    let escalate = input(json!({"action": "escalate", "id": id, "note": "help"}));
    assert_eq!(escalate.action, InboxAction::Escalate);
    assert_eq!(escalate.id, Some(id));
    assert_eq!(escalate.note.as_deref(), Some("help"));
}

#[tokio::test]
async fn a_new_user_lists_an_empty_feed() {
    let r = rig(false).await;
    let user = registered("inbox").await;
    let ctx = ctx_for(Some(&user));
    let (output, summary) = call(
        &r.tools,
        &ctx,
        json!({"action": "list", "limit": 500, "unread": true, "attention": "agent"}),
    )
    .await
    .expect("list");
    let InboxOutput::Page(page) = output else {
        panic!("expected a page");
    };
    assert!(page.notifications.is_empty());
    assert_eq!(page.unread, 0);
    assert!(page.next_cursor.is_none());
    assert_eq!(summary, "0 notifications, 0 unread");
}

#[tokio::test]
async fn mark_read_all_answers_the_unread_count() {
    let r = rig(false).await;
    let user = registered("inbox-read").await;
    let ctx = ctx_for(Some(&user));
    let (output, summary) = call(&r.tools, &ctx, json!({"action": "mark_read", "all": true}))
        .await
        .expect("mark_read");
    let InboxOutput::Read { ids, unread } = output else {
        panic!("expected a read ack");
    };
    assert!(ids.is_empty());
    assert_eq!(unread, 0);
    assert_eq!(summary, "0 marked read, 0 unread");
}

#[tokio::test]
async fn inbox_refuses_malformed_calls() {
    let r = rig(false).await;
    let user = registered("inbox-bad").await;
    let ctx = ctx_for(Some(&user));
    let cases = [
        (
            json!({"action": "list", "since": "not-a-cursor"}),
            "is not valid",
        ),
        (json!({"action": "mark_read"}), "needs ids or all"),
        (json!({"action": "escalate"}), "escalate needs id"),
    ];
    for (value, needle) in cases {
        let error = call(&r.tools, &ctx, value).await.expect_err(needle);
        assert_eq!(code_of(&error), RefusalCode::InvalidParams);
        assert!(error.message.contains(needle), "{}", error.message);
    }
}

#[tokio::test]
async fn escalating_an_unknown_notification_is_not_found() {
    let r = rig(false).await;
    let user = registered("inbox-esc").await;
    let ctx = ctx_for(Some(&user));
    let error = call(
        &r.tools,
        &ctx,
        json!({"action": "escalate", "id": NotificationId::generate(), "note": "why"}),
    )
    .await
    .expect_err("unknown");
    assert_eq!(code_of(&error), RefusalCode::NotFound);
}

#[tokio::test]
async fn the_inbox_acts_only_for_a_signed_in_user() {
    let r = rig(false).await;
    let error = call(&r.tools, &ctx_for(None), json!({"action": "list"}))
        .await
        .expect_err("anonymous");
    assert_eq!(code_of(&error), RefusalCode::Forbidden);
}
