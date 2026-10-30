use base64::Engine;
use serde_json::json;
use systemprompt_identifiers::DocumentUploadId;
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::content_hash;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_runtime::documents::mcp::{
    DocumentsMcpTools, UploadHandler, UploadInput, UploadMode, UploadOutput,
};
use systemprompt_runtime::documents::refusal::RefusalCode;
use systemprompt_runtime::documents::upload_links::UPLOAD_TOKEN_HEADER;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{ctx_for, exec, registered};

async fn call(
    tools: &DocumentsMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<(UploadOutput, String), rmcp::ErrorData> {
    let input: UploadInput = serde_json::from_value(value).expect("upload input");
    UploadHandler { tools }.handle(input, ctx, &exec()).await
}

fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn an_inline_body_answers_its_blob_and_repeats_deduplicate() {
    let r = rig(true).await;
    let user = registered("up-inline").await;
    let ctx = ctx_for(Some(&user));
    let bytes = format!("hello {}", uuid::Uuid::new_v4()).into_bytes();
    let body =
        json!({"name": "notes.txt", "mime": "text/plain", "content_base64": encoded(&bytes)});
    let (first, summary) = call(&r.tools, &ctx, body.clone()).await.expect("inline");
    assert_eq!(first.mode, UploadMode::Inline);
    assert_eq!(first.blob_ref, Some(content_hash(&bytes)));
    assert_eq!(first.deduplicated, Some(false));
    assert!(first.parts_url.is_none() && first.upload_token.is_none());
    assert!(
        summary.contains(&format!("({} bytes)", bytes.len())),
        "{summary}"
    );
    let (second, _) = call(&r.tools, &ctx, body).await.expect("again");
    assert_eq!(second.blob_ref, first.blob_ref);
    assert_eq!(second.deduplicated, Some(true));
    assert_ne!(second.upload_id, first.upload_id);
}

#[tokio::test]
async fn a_declared_upload_answers_signed_part_links() {
    let r = rig(true).await;
    let user = registered("up-resume").await;
    let ctx = ctx_for(Some(&user));
    let bytes = vec![7u8; 1024];
    let (session, summary) = call(
        &r.tools,
        &ctx,
        json!({"name": "big.bin", "mime": "application/octet-stream",
               "size": 1024, "sha256": content_hash(&bytes)}),
    )
    .await
    .expect("resumable");
    assert_eq!(session.mode, UploadMode::Resumable);
    assert_eq!(session.part_count, Some(1));
    assert!(session.blob_ref.is_none());
    let parts = session.parts_url.as_deref().expect("parts_url");
    assert!(parts.starts_with("https://docs.example.test/"), "{parts}");
    assert!(parts.contains(session.upload_id.as_str()), "{parts}");
    assert!(session.status_url.is_some() && session.complete_url.is_some());
    assert_eq!(session.token_header.as_deref(), Some(UPLOAD_TOKEN_HEADER));
    assert!(session.expires_at.is_some());
    assert!(summary.contains(UPLOAD_TOKEN_HEADER), "{summary}");

    let (again, _) = call(&r.tools, &ctx, json!({"upload_id": session.upload_id}))
        .await
        .expect("reissue");
    assert_eq!(again.upload_id, session.upload_id);
    assert_eq!(again.mode, UploadMode::Resumable);
    assert!(again.upload_token.is_some());
}

#[tokio::test]
async fn reissuing_a_finished_or_unknown_upload_is_refused() {
    let r = rig(true).await;
    let user = registered("up-reissue").await;
    let ctx = ctx_for(Some(&user));
    let bytes = format!("done {}", uuid::Uuid::new_v4()).into_bytes();
    let (done, _) = call(
        &r.tools,
        &ctx,
        json!({"name": "a.txt", "mime": "text/plain", "content_base64": encoded(&bytes)}),
    )
    .await
    .expect("inline");
    let closed = call(&r.tools, &ctx, json!({"upload_id": done.upload_id}))
        .await
        .expect_err("closed");
    assert_eq!(code_of(&closed), RefusalCode::InvalidParams);
    let unknown = call(
        &r.tools,
        &ctx,
        json!({"upload_id": DocumentUploadId::generate()}),
    )
    .await
    .expect_err("unknown");
    assert_eq!(code_of(&unknown), RefusalCode::NotFound);
}

#[tokio::test]
async fn malformed_upload_calls_are_refused() {
    let r = rig(true).await;
    let user = registered("up-bad").await;
    let ctx = ctx_for(Some(&user));
    let cases = [
        (
            json!({"name": "a"}),
            "pass content_base64",
            RefusalCode::InvalidParams,
        ),
        (
            json!({"name": "a", "size": 3}),
            "pass content_base64",
            RefusalCode::InvalidParams,
        ),
        (
            json!({"name": "a", "mime": "text/plain", "content_base64": "!!!not base64"}),
            "not valid base64",
            RefusalCode::InvalidParams,
        ),
    ];
    for (value, needle, code) in cases {
        let error = call(&r.tools, &ctx, value).await.expect_err(needle);
        assert_eq!(code_of(&error), code);
        assert!(error.message.contains(needle), "{}", error.message);
    }
    let huge = "A".repeat(8 * 1024 * 1024);
    let error = call(
        &r.tools,
        &ctx,
        json!({"name": "big", "mime": "text/plain", "content_base64": huge}),
    )
    .await
    .expect_err("too large");
    assert_eq!(code_of(&error), RefusalCode::TooLarge);
    assert!(error.message.contains("resumable"), "{}", error.message);
}

#[tokio::test]
async fn a_server_without_uploads_refuses_and_anonymous_is_forbidden() {
    let r = rig(false).await;
    let user = registered("up-none").await;
    let error = call(
        &r.tools,
        &ctx_for(Some(&user)),
        json!({"name": "a", "mime": "text/plain", "content_base64": encoded(b"x")}),
    )
    .await
    .expect_err("no uploads");
    assert_eq!(code_of(&error), RefusalCode::InvalidParams);
    assert!(error.message.contains("does not accept uploads"));
    let anonymous = call(&r.tools, &ctx_for(None), json!({"name": "a"}))
        .await
        .expect_err("anonymous");
    assert_eq!(code_of(&anonymous), RefusalCode::Forbidden);
}
