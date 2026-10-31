use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_runtime::documents::mcp::{ExportAs, ExportFormat, ExportHandler, ExportInput};
use systemprompt_runtime::documents::refusal::RefusalCode;

use crate::documents_mcp_rig::code_of;
use crate::documents_mcp_support::{create_text, ctx_for, exec, harness, registered};

fn input(
    document: &DocumentId,
    revision: Option<DocumentRevisionId>,
    format: ExportFormat,
    export_as: ExportAs,
) -> ExportInput {
    ExportInput {
        document_id: document.clone(),
        revision_id: revision,
        format,
        export_as,
    }
}

#[tokio::test]
async fn formats_a_text_document_cannot_link_are_refused() {
    let h = harness().await;
    let owner = registered("export-refuse").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "hello").await;
    let tool = ExportHandler { tools: &h.tools };

    for (format, export_as, needle) in [
        (
            ExportFormat::Pdf,
            ExportAs::Preview,
            "pdf exports as url only",
        ),
        (
            ExportFormat::Md,
            ExportAs::Url,
            "exports as url only in its own format",
        ),
        (
            ExportFormat::Html,
            ExportAs::Url,
            "exports as url only in its own format",
        ),
    ] {
        let refused = tool
            .handle(input(&document, None, format, export_as), &ctx, &exec())
            .await
            .expect_err("unsupported export refused");
        assert_eq!(code_of(&refused), RefusalCode::UnsupportedFormat);
        assert!(refused.message.contains(needle), "{}", refused.message);
    }
}

#[tokio::test]
async fn an_explicit_revision_is_exported() {
    let h = harness().await;
    let owner = registered("export-revision").await;
    let ctx = ctx_for(Some(&owner));
    let (document, revision) = create_text(&h.tools, &ctx, "pinned").await;

    let (output, summary) = ExportHandler { tools: &h.tools }
        .handle(
            input(
                &document,
                Some(revision.clone()),
                ExportFormat::Native,
                ExportAs::Preview,
            ),
            &ctx,
            &exec(),
        )
        .await
        .expect("preview of a pinned revision");

    assert_eq!(output.revision_id, revision);
    assert_eq!(summary, format!("preview of revision {revision}"));
    assert!(output.url.is_none());
    assert!(
        output
            .html
            .as_deref()
            .is_some_and(|html| html.contains("pinned"))
    );
}

#[tokio::test]
async fn an_unknown_revision_is_refused() {
    let h = harness().await;
    let owner = registered("export-missing").await;
    let ctx = ctx_for(Some(&owner));
    let (document, _) = create_text(&h.tools, &ctx, "hello").await;

    let refused = ExportHandler { tools: &h.tools }
        .handle(
            input(
                &document,
                Some(DocumentRevisionId::generate()),
                ExportFormat::Native,
                ExportAs::Url,
            ),
            &ctx,
            &exec(),
        )
        .await
        .expect_err("unknown revision refused");

    assert_eq!(code_of(&refused), RefusalCode::NotFound);
}
