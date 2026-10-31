use serde_json::json;
use systemprompt_identifiers::{DocumentId, DocumentNodeId, DocumentRevisionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_provider_contracts::documents::{EditOp, InsertBlock, InsertImage, NodeTarget};
use systemprompt_runtime::documents::mcp::{DocumentsMcpTools, EditHandler, EditInput};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_rig::{code_of, rig};
use crate::documents_mcp_support::{create_text, ctx_for, exec, registered};

fn anchor() -> NodeTarget {
    NodeTarget::Id(DocumentNodeId::new("n-anchor"))
}

fn image(blob: &str) -> EditOp {
    EditOp::InsertImage(InsertImage {
        after: anchor(),
        blob: blob.to_owned(),
        alt: String::new(),
    })
}

fn image_block(blob: &str) -> EditOp {
    EditOp::InsertBlock(InsertBlock {
        after: anchor(),
        block: json!({ "type": "image", "blob": blob }),
    })
}

async fn edit(
    tools: &DocumentsMcpTools,
    ctx: &systemprompt_models::execution::context::RequestContext,
    document: &DocumentId,
    base: DocumentRevisionId,
    ops: Vec<EditOp>,
) -> Result<(), rmcp::ErrorData> {
    EditHandler { tools }
        .handle(
            EditInput {
                document_id: document.clone(),
                base_revision: Some(base),
                mode: None,
                op_id: None,
                answering: None,
                allow_detach: false,
                ops,
                message: None,
                verbose: false,
            },
            ctx,
            &exec(),
        )
        .await
        .map(|_| ())
}

async fn assert_unheld_refused(with_uploads: bool, op: EditOp, blob: &str) {
    let r = rig(with_uploads).await;
    let user = registered("media-owner").await;
    let ctx = ctx_for(Some(&user));
    let (document, base) = create_text(&r.tools, &ctx, "body\n").await;

    let refused = edit(&r.tools, &ctx, &document, base, vec![op])
        .await
        .expect_err("unheld image refused");

    assert_eq!(code_of(&refused), RefusalCode::InvalidParams);
    assert!(
        refused
            .message
            .contains(&format!("image {blob} is neither your upload")),
        "{}",
        refused.message
    );
}

#[tokio::test]
async fn an_image_op_naming_a_foreign_blob_is_refused_with_uploads() {
    assert_unheld_refused(true, image("blob-not-mine-1"), "blob-not-mine-1").await;
}

#[tokio::test]
async fn an_image_op_naming_a_foreign_blob_is_refused_without_uploads() {
    assert_unheld_refused(false, image("blob-not-mine-2"), "blob-not-mine-2").await;
}

#[tokio::test]
async fn an_image_block_naming_a_foreign_blob_is_refused() {
    assert_unheld_refused(true, image_block("blob-not-mine-3"), "blob-not-mine-3").await;
}

#[tokio::test]
async fn a_url_image_needs_no_upload() {
    let r = rig(false).await;
    let user = registered("media-url").await;
    let ctx = ctx_for(Some(&user));
    let (document, base) = create_text(&r.tools, &ctx, "body\n").await;

    let outcome = edit(
        &r.tools,
        &ctx,
        &document,
        base,
        vec![image("https://img.example.test/a.png")],
    )
    .await;

    if let Err(error) = outcome {
        assert!(
            !error.message.contains("is neither your upload"),
            "{}",
            error.message
        );
    }
}
