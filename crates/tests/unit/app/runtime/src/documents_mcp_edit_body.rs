use serde_json::json;
use systemprompt_extension::mcp::McpToolHandler;
use systemprompt_files::documents::agents::{AgentCap, DocumentActor, act_as};
use systemprompt_files::documents::service::AgentCapTarget;
use systemprompt_files::uploads::NewUpload;
use systemprompt_identifiers::{AgentName, ContentHash, DocumentId, DocumentRevisionId, TraceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::content_hash;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    CommentHandler, CommentInput, DocumentsMcpTools, EditHandler, EditInput, ImportHandler,
    ImportInput, refusal_result,
};
use systemprompt_runtime::documents::refusal::RefusalCode;
use systemprompt_runtime::documents::views::EditOutput;

use crate::documents_mcp_rig::{Rig, code_of, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, line_for, read_text, registered};

const PAGE: &str = "<!doctype html><html><head><title>App</title></head><body>\
<h1 class=\"title\">Report</h1><p onclick=\"go()\">Body</p>\
<script>document.body.dataset.ready = '1'</script></body></html>";

fn page(marker: &str) -> String {
    format!("{PAGE}<!-- {marker} {} -->", uuid::Uuid::new_v4().simple())
}

async fn uploaded(r: &Rig, user: &AuthenticatedUser, name: &str, bytes: &[u8]) -> ContentHash {
    let mime = if name.ends_with(".html") {
        "text/html"
    } else {
        "text/plain"
    };
    r.runtime
        .uploads()
        .put_inline(
            &user.id,
            NewUpload {
                name: name.to_owned(),
                mime: mime.to_owned(),
                size: bytes.len() as u64,
                sha256: content_hash(bytes),
            },
            bytes.to_vec(),
            &TraceId::generate(),
        )
        .await
        .expect("upload")
        .blob
}

async fn imported_page(r: &Rig, user: &AuthenticatedUser, ctx: &RequestContext) -> DocumentId {
    let blob = uploaded(r, user, "page.html", page("first").as_bytes()).await;
    let input: ImportInput = serde_json::from_value(json!({
        "blob_ref": blob,
        "name": format!("page-{}.html", uuid::Uuid::new_v4().simple()),
    }))
    .expect("import input");
    ImportHandler { tools: &r.tools }
        .handle(input, ctx, &exec())
        .await
        .expect("import")
        .0
        .document_id
}

async fn edit(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<(EditOutput, String), rmcp::ErrorData> {
    let input: EditInput = serde_json::from_value(value).expect("edit input");
    EditHandler { tools }.handle(input, ctx, &exec()).await
}

async fn as_agent<F: std::future::Future>(work: F) -> F::Output {
    act_as(
        DocumentActor::agent(AgentName::new("body-agent"), false),
        work,
    )
    .await
}

async fn head(r: &Rig, user: &AuthenticatedUser, document: &DocumentId) -> DocumentRevisionId {
    r.tools
        .service()
        .read_bytes(user, document, None, &TraceId::generate())
        .await
        .expect("read")
        .0
        .id
}

async fn head_bytes(r: &Rig, user: &AuthenticatedUser, document: &DocumentId) -> Vec<u8> {
    r.tools
        .service()
        .read_bytes(user, document, None, &TraceId::generate())
        .await
        .expect("read")
        .1
}

#[tokio::test]
async fn a_whole_file_replace_lands_the_uploaded_bytes_verbatim() {
    let r = rig(true).await;
    let user = registered("body-person").await;
    let ctx = ctx_for(Some(&user));
    let document = imported_page(&r, &user, &ctx).await;
    let next = page("second");
    let blob = uploaded(&r, &user, "page.html", next.as_bytes()).await;
    let (ack, _) = edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await, "message": "rewrite"}),
    )
    .await
    .expect("a person replaces the whole file");
    assert!(ack.proposal_id.is_none());
    assert_eq!(head_bytes(&r, &user, &document).await, next.as_bytes());

    let ops_on_scripts = edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "ops": [{"op": "str_replace", "old_str": "Body", "new_str": "Text"}]}),
    )
    .await
    .expect_err("str_replace on a scripted page");
    assert_eq!(code_of(&ops_on_scripts), RefusalCode::UnsupportedOp);
    assert!(
        ops_on_scripts.message.contains("content_blob_ref"),
        "{}",
        ops_on_scripts.message
    );
    assert_eq!(head_bytes(&r, &user, &document).await, next.as_bytes());
}

#[tokio::test]
async fn an_agent_needs_replace_content_to_land_a_whole_file() {
    let r = rig(true).await;
    let user = registered("body-agent").await;
    let ctx = ctx_for(Some(&user));
    let document = imported_page(&r, &user, &ctx).await;
    let before = head_bytes(&r, &user, &document).await;
    let blob = uploaded(&r, &user, "page.html", page("agent").as_bytes()).await;
    let refused = as_agent(edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await}),
    ))
    .await
    .expect_err("the default policy allows no replace_content");
    assert_eq!(
        code_of(&refused),
        RefusalCode::AgentForbidden,
        "{}",
        refused.message
    );
    assert_eq!(head_bytes(&r, &user, &document).await, before);
}

#[tokio::test]
async fn a_suggest_cap_or_propose_mode_writes_the_whole_file_as_a_proposal() {
    let r = rig(true).await;
    let user = registered("body-suggest").await;
    let ctx = ctx_for(Some(&user));
    let document = imported_page(&r, &user, &ctx).await;
    let before = head_bytes(&r, &user, &document).await;

    let asked = page("asked");
    let blob = uploaded(&r, &user, "page.html", asked.as_bytes()).await;
    let (ack, summary) = edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await, "mode": "propose"}),
    )
    .await
    .expect("propose a whole file");
    assert!(ack.proposal_id.is_some(), "{summary}");
    assert!(
        !summary.contains("converted") && !summary.contains("not landed"),
        "{summary}"
    );
    let view = r
        .tools
        .service()
        .proposal_view(
            &user,
            ack.proposal_id.as_ref().expect("proposal"),
            &TraceId::generate(),
        )
        .await
        .expect("proposal view");
    assert_eq!(view.proposed.content_sha256, content_hash(asked.as_bytes()));
    assert_eq!(head_bytes(&r, &user, &document).await, before);

    r.tools
        .service()
        .set_agent_cap(
            &user,
            AgentCapTarget::Document(document.clone()),
            Some(AgentCap::Suggest),
            &TraceId::generate(),
        )
        .await
        .expect("cap the document");
    let blob = uploaded(&r, &user, "page.html", page("capped").as_bytes()).await;
    let (ack, summary) = as_agent(edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await}),
    ))
    .await
    .expect("a suggest-capped agent proposes");
    assert!(ack.proposal_id.is_some(), "{summary}");
    assert!(summary.contains("not landed"), "{summary}");
    assert_eq!(head_bytes(&r, &user, &document).await, before);
}

#[tokio::test]
async fn a_whole_file_replace_guards_comment_anchors() {
    let r = rig(true).await;
    let user = registered("body-anchors").await;
    let ctx = ctx_for(Some(&user));
    let (document, _) = create_text(&r.tools, &ctx, "alpha beta\nsecond\n").await;
    let read = read_text(&r.tools, &ctx, &document).await;
    let node = line_for(&read, "alpha beta").0.to_owned();
    let comment: CommentInput = serde_json::from_value(json!({
        "document_id": document,
        "action": "create",
        "node": node,
        "anchor": {"find": "beta"},
        "body": "why beta?",
    }))
    .expect("comment input");
    CommentHandler { tools: &r.tools }
        .handle(comment, &ctx, &exec())
        .await
        .expect("comment");
    let blob = uploaded(&r, &user, "notes.txt", b"second\n").await;
    let refused = edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await}),
    )
    .await
    .expect_err("the anchored line is gone");
    let data = refusal_result(&refused)
        .expect("refusal")
        .structured_content
        .expect("structured");
    assert_eq!(data["code"], "anchors_affected");
    edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await, "allow_detach": true}),
    )
    .await
    .expect("detach allowed");
    assert_eq!(head_bytes(&r, &user, &document).await, b"second\n");
}

#[tokio::test]
async fn ops_and_content_blob_ref_are_exclusive_and_one_is_required() {
    let r = rig(true).await;
    let user = registered("body-params").await;
    let ctx = ctx_for(Some(&user));
    let (document, _) = create_text(&r.tools, &ctx, "alpha\n").await;
    let blob = uploaded(&r, &user, "notes.txt", b"beta\n").await;
    let neither = edit(&r.tools, &ctx, json!({"document_id": document}))
        .await
        .expect_err("neither");
    assert_eq!(code_of(&neither), RefusalCode::InvalidParams);
    assert!(
        neither.message.contains("content_blob_ref"),
        "{}",
        neither.message
    );
    let both = edit(
        &r.tools,
        &ctx,
        json!({
            "document_id": document,
            "content_blob_ref": blob, "base_revision": head(&r, &user, &document).await,
            "ops": [{"op": "append_markdown", "markdown": "x"}],
        }),
    )
    .await
    .expect_err("both");
    assert_eq!(code_of(&both), RefusalCode::InvalidParams);
    assert!(both.message.contains("exclusive"), "{}", both.message);
    let unbased = edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": blob}),
    )
    .await
    .expect_err("no base revision");
    assert_eq!(code_of(&unbased), RefusalCode::InvalidParams);
    assert!(
        unbased.message.contains("base_revision"),
        "{}",
        unbased.message
    );
    let other = registered("body-params-other").await;
    let foreign = uploaded(&r, &other, "notes.txt", b"theirs\n").await;
    let stolen = edit(
        &r.tools,
        &ctx,
        json!({"document_id": document, "content_blob_ref": foreign, "base_revision": head(&r, &user, &document).await}),
    )
    .await
    .expect_err("another user's blob");
    assert_eq!(code_of(&stolen), RefusalCode::NotFound);
}
