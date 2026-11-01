use serde_json::json;
use systemprompt_identifiers::{DocumentCommentId, DocumentId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{CommentHandler, CommentInput, DocumentsMcpTools};
use systemprompt_runtime::documents::refusal::RefusalCode;
use systemprompt_runtime::documents::views::CommentOutput;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, line_for, read_text, registered};

fn input(document: &DocumentId, extra: serde_json::Value) -> CommentInput {
    let mut value = json!({ "document_id": document });
    for (key, field) in extra.as_object().expect("object") {
        value[key] = field.clone();
    }
    serde_json::from_value(value).expect("comment input")
}

async fn anchored(tools: &DocumentsMcpTools, ctx: &RequestContext) -> (DocumentId, String) {
    let (document, _) = create_text(tools, ctx, "alpha").await;
    let read = read_text(tools, ctx, &document).await;
    let node = line_for(&read, "alpha").0.to_owned();
    (document, node)
}

async fn call(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    input: CommentInput,
) -> Result<CommentOutput, rmcp::ErrorData> {
    CommentHandler { tools }
        .handle(input, ctx, &exec())
        .await
        .map(|(output, _)| output)
}

#[tokio::test]
async fn a_mention_by_id_becomes_a_token_and_lists_the_person() {
    let r = rig(false).await;
    let owner = registered("c-owner").await;
    let mate = registered("c-mate").await;
    let ctx = ctx_for(Some(&owner));
    let (document, node) = anchored(&r.tools, &ctx).await;
    let body = format!("ping @{} please", mate.id);
    let created = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"action": "create", "node": node, "body": body, "mentions": [mate.id]}),
        ),
    )
    .await
    .expect("create");
    let comment = &created.comments[0];
    assert_eq!(comment.mentions, vec![mate.id.clone()]);
    assert!(
        comment.body.contains(&format!("](user:{})", mate.id)),
        "{}",
        comment.body
    );
    assert!(comment.body.starts_with("ping @["), "{}", comment.body);
    assert!(comment.body.ends_with(" please"), "{}", comment.body);
}

#[tokio::test]
async fn a_mention_by_email_not_in_the_body_is_appended() {
    let r = rig(false).await;
    let owner = registered("c-owner").await;
    let mate = registered("c-mail").await;
    let ctx = ctx_for(Some(&owner));
    let (document, node) = anchored(&r.tools, &ctx).await;
    let created = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"action": "create", "node": node, "body": "see this", "mentions": [mate.email.to_uppercase()]}),
        ),
    )
    .await
    .expect("create");
    let comment = &created.comments[0];
    assert_eq!(comment.mentions, vec![mate.id.clone()]);
    assert!(comment.body.starts_with("see this @["), "{}", comment.body);
}

#[tokio::test]
async fn unknown_and_excess_mentions_are_refused() {
    let r = rig(false).await;
    let owner = registered("c-owner").await;
    let ctx = ctx_for(Some(&owner));
    let (document, node) = anchored(&r.tools, &ctx).await;
    let unknown = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"action": "create", "node": node, "body": "x", "mentions": ["nobody-at-all"]}),
        ),
    )
    .await
    .expect_err("unknown");
    assert_eq!(code_of(&unknown), RefusalCode::UnknownSubject);
    assert!(unknown.message.contains("documents_people"));
    let many: Vec<String> = (0..21).map(|n| format!("user-{n}")).collect();
    let excess = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"action": "create", "node": node, "body": "x", "mentions": many}),
        ),
    )
    .await
    .expect_err("excess");
    assert_eq!(code_of(&excess), RefusalCode::InvalidParams);
    assert!(excess.message.contains("at most 20"));
}

#[tokio::test]
async fn edit_rewrites_the_body_and_delete_removes_the_comment() {
    let r = rig(false).await;
    let owner = registered("c-edit").await;
    let ctx = ctx_for(Some(&owner));
    let (document, node) = anchored(&r.tools, &ctx).await;
    let created = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"action": "create", "node": node, "body": "first"}),
        ),
    )
    .await
    .expect("create");
    let id = created.comments[0].id.clone();
    let edited = call(
        &r.tools,
        &ctx,
        input(
            &document,
            json!({"action": "edit", "comment_id": id, "body": "second"}),
        ),
    )
    .await
    .expect("edit");
    assert_eq!(edited.comments[0].id, id);
    assert_eq!(edited.comments[0].body, "second");
    assert!(edited.comments[0].edited_at.is_some());
    let listed = call(&r.tools, &ctx, input(&document, json!({"action": "list"})))
        .await
        .expect("list");
    assert_eq!(listed.comments.len(), 1);
    let after = call(
        &r.tools,
        &ctx,
        input(&document, json!({"action": "delete", "comment_id": id})),
    )
    .await
    .expect("delete");
    assert!(after.comments.iter().all(|c| c.id != id));
}

#[tokio::test]
async fn edit_and_delete_refuse_missing_arguments() {
    let r = rig(false).await;
    let owner = registered("c-args").await;
    let ctx = ctx_for(Some(&owner));
    let (document, node) = anchored(&r.tools, &ctx).await;
    let cases = [
        (
            json!({"action": "edit", "body": "x"}),
            "edit needs comment_id",
        ),
        (
            json!({"action": "edit", "comment_id": DocumentCommentId::generate(), "body": "  "}),
            "non-empty body",
        ),
        (json!({"action": "delete"}), "delete needs comment_id"),
        (json!({"action": "resolve"}), "resolve needs comment_id"),
        (
            json!({"action": "create", "node": node, "body": ""}),
            "non-empty body",
        ),
    ];
    for (extra, needle) in cases {
        let error = call(&r.tools, &ctx, input(&document, extra))
            .await
            .expect_err(needle);
        assert_eq!(code_of(&error), RefusalCode::InvalidParams);
        assert!(error.message.contains(needle), "{}", error.message);
    }
}

#[tokio::test]
async fn a_comment_without_a_user_is_forbidden() {
    let r = rig(false).await;
    let error = call(
        &r.tools,
        &ctx_for(None),
        input(&DocumentId::generate(), json!({"action": "list"})),
    )
    .await
    .expect_err("anonymous");
    assert_eq!(code_of(&error), RefusalCode::Forbidden);
}
