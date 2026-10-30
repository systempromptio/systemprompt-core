use std::sync::Arc;

use serde_json::json;
use systemprompt_files::uploads::NewUpload;
use systemprompt_identifiers::{DocumentUploadId, IssueReportId, McpServerId};
use systemprompt_mcp::McpToolHandler;
use systemprompt_models::documents::content_hash;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::issues::{IssueKind, IssueSeverity, IssueStatus};
use systemprompt_runtime::documents::upload_links::UPLOAD_TOKEN_HEADER;
use systemprompt_runtime::issues::mcp::IssuesMcpTools;
use systemprompt_runtime::issues::mcp::attach::{AttachmentHandler, AttachmentInput};
use systemprompt_runtime::issues::mcp::read::{ListHandler, ListInput, StatusHandler, StatusInput};
use systemprompt_runtime::issues::mcp::report::{ReportHandler, ReportInput, ReportOutput};

use crate::documents_mcp_rig::{Rig, rig};
use crate::documents_mcp_support::{ctx_for, exec, registered};

fn tools(r: &Rig, with_uploads: bool) -> IssuesMcpTools {
    IssuesMcpTools::new(
        Arc::clone(r.runtime.issues()),
        with_uploads.then(|| Arc::clone(r.runtime.uploads())),
        systemprompt_test_fixtures::test_token_authority("issues-test"),
        McpServerId::new("issues"),
        "https://issues.example.test/".to_owned(),
    )
}

fn code(error: &rmcp::ErrorData) -> String {
    error
        .data
        .as_ref()
        .and_then(|data| data["code"].as_str())
        .unwrap_or_else(|| panic!("not a refusal: {error:?}"))
        .to_owned()
}

async fn report(
    tools: &IssuesMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<(ReportOutput, String), rmcp::ErrorData> {
    let input: ReportInput = serde_json::from_value(value).expect("report input");
    ReportHandler { tools }.handle(input, ctx, &exec()).await
}

async fn attach(
    tools: &IssuesMcpTools,
    ctx: &RequestContext,
    value: serde_json::Value,
) -> Result<systemprompt_runtime::issues::mcp::attach::AttachmentOutput, rmcp::ErrorData> {
    let input: AttachmentInput = serde_json::from_value(value).expect("attachment input");
    AttachmentHandler { tools }
        .handle(input, ctx, &exec())
        .await
        .map(|(output, _)| output)
}

#[test]
fn issue_inputs_refuse_unknown_keys() {
    assert!(
        serde_json::from_value::<ReportInput>(json!({
            "kind": "bug", "severity": "low", "title": "t", "extra": 1
        }))
        .is_err()
    );
    assert!(serde_json::from_value::<ListInput>(json!({"page": 2})).is_err());
    assert!(serde_json::from_value::<StatusInput>(json!({})).is_err());
}

#[tokio::test]
async fn a_report_records_the_call_and_a_repeat_folds_into_it() {
    let r = rig(false).await;
    let t = tools(&r, false);
    let user = registered("issue-file").await;
    let ctx = ctx_for(Some(&user));
    let title = format!("tool keeps failing {}", uuid::Uuid::new_v4().simple());
    let body = json!({"kind": "bug", "severity": "high", "title": title,
                      "body": "did x", "client": {"os": "linux"}});
    let (first, summary) = report(&t, &ctx, body.clone()).await.expect("file");
    assert!(!first.folded);
    assert_eq!(first.occurrences, 1);
    assert_eq!(summary, format!("filed issue {}", first.report_id));
    let (second, summary) = report(&t, &ctx, body).await.expect("fold");
    assert!(second.folded);
    assert_eq!(second.report_id, first.report_id);
    assert_eq!(second.occurrences, 2);
    assert!(summary.contains("2 occurrences"), "{summary}");

    let (status, summary) = StatusHandler { tools: &t }
        .handle(
            StatusInput {
                report_id: first.report_id.clone(),
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("status");
    let found = status.report;
    assert_eq!(found.kind, IssueKind::Bug);
    assert_eq!(found.severity, IssueSeverity::High);
    assert_eq!(found.status, IssueStatus::Open);
    assert_eq!(found.reporter_user_id, user.id);
    assert_eq!(found.context.trace.as_ref(), Some(ctx.trace_id()));
    assert_eq!(found.context.session.as_ref(), Some(ctx.session_id()));
    assert!(found.context.mcp_execution.is_some());
    assert_eq!(found.client, json!({"os": "linux"}));
    assert!(found.via_agent.is_some());
    assert_eq!(summary, format!("issue {} is open", first.report_id));

    let (listed, summary) = ListHandler { tools: &t }
        .handle(
            serde_json::from_value(json!({"status": "open", "limit": 0})).expect("list"),
            &ctx,
            &exec(),
        )
        .await
        .expect("list");
    assert_eq!(listed.reports.len(), 1);
    assert_eq!(listed.reports[0].id, first.report_id);
    assert_eq!(summary, "1 issue reports");
}

#[tokio::test]
async fn reports_refuse_bad_input_and_strangers() {
    let r = rig(false).await;
    let t = tools(&r, false);
    let user = registered("issue-bad").await;
    let ctx = ctx_for(Some(&user));
    let blank = report(
        &t,
        &ctx,
        json!({"kind": "feedback", "severity": "low", "title": "  "}),
    )
    .await
    .expect_err("blank title");
    assert_eq!(code(&blank), "invalid_params");
    let anonymous = report(
        &t,
        &ctx_for(None),
        json!({"kind": "bug", "severity": "low", "title": "x"}),
    )
    .await
    .expect_err("anonymous");
    assert_eq!(code(&anonymous), "forbidden");
    let stranger = registered("issue-stranger").await;
    let missing = StatusHandler { tools: &t }
        .handle(
            StatusInput {
                report_id: IssueReportId::generate(),
            },
            &ctx_for(Some(&stranger)),
            &exec(),
        )
        .await
        .expect_err("unknown");
    assert_eq!(code(&missing), "not_found");
}

#[tokio::test]
async fn an_attachment_is_declared_then_attached_once_uploaded() {
    let r = rig(true).await;
    let t = tools(&r, true);
    let user = registered("issue-attach").await;
    let ctx = ctx_for(Some(&user));
    let (filed, _) = report(
        &t,
        &ctx,
        json!({"kind": "bug", "severity": "low", "title": format!("a {}", uuid::Uuid::new_v4())}),
    )
    .await
    .expect("file");
    let bytes = format!("zip {}", uuid::Uuid::new_v4()).into_bytes();
    let declared = attach(
        &t,
        &ctx,
        json!({"action": "declare", "report_id": filed.report_id, "name": "diag.zip",
               "mime": "application/zip", "size": bytes.len(), "sha256": content_hash(&bytes)}),
    )
    .await
    .expect("declare");
    assert!(!declared.attached);
    assert!(
        declared
            .parts_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://issues.example.test/"))
    );
    assert_eq!(declared.token_header.as_deref(), Some(UPLOAD_TOKEN_HEADER));
    assert!(declared.upload_token.is_some() && declared.expires_at.is_some());

    let not_ready = attach(
        &t,
        &ctx,
        json!({"action": "attach", "report_id": filed.report_id, "upload_id": declared.upload_id}),
    )
    .await
    .expect_err("not complete");
    assert_eq!(code(&not_ready), "upload_not_ready");

    let done = r
        .runtime
        .uploads()
        .put_inline(
            &user.id,
            NewUpload {
                name: "diag.zip".to_owned(),
                mime: "application/zip".to_owned(),
                size: bytes.len() as u64,
                sha256: content_hash(&bytes),
            },
            bytes.clone(),
            ctx.trace_id(),
        )
        .await
        .expect("inline upload");
    let attached = attach(
        &t,
        &ctx,
        json!({"action": "attach", "report_id": filed.report_id, "upload_id": done.session.id}),
    )
    .await
    .expect("attach");
    assert!(attached.attached);
    assert!(attached.parts_url.is_none() && attached.upload_token.is_none());
    let (status, _) = StatusHandler { tools: &t }
        .handle(
            StatusInput {
                report_id: filed.report_id.clone(),
            },
            &ctx,
            &exec(),
        )
        .await
        .expect("status");
    assert_eq!(status.report.attachments.len(), 1);
}

#[tokio::test]
async fn declaring_refuses_missing_fields_and_an_install_without_uploads() {
    let r = rig(true).await;
    let t = tools(&r, true);
    let user = registered("issue-declare").await;
    let ctx = ctx_for(Some(&user));
    let (filed, _) = report(
        &t,
        &ctx,
        json!({"kind": "bug", "severity": "low", "title": format!("d {}", uuid::Uuid::new_v4())}),
    )
    .await
    .expect("file");
    let id = filed.report_id;
    let cases = [
        (
            json!({"action": "declare", "report_id": id}),
            "sha256 is required",
        ),
        (
            json!({"action": "declare", "report_id": id, "sha256": "XYZ", "name": "a", "mime": "a/b", "size": 1}),
            "64 lowercase hex",
        ),
        (
            json!({"action": "declare", "report_id": id, "sha256": "0".repeat(64)}),
            "name is required",
        ),
        (
            json!({"action": "attach", "report_id": id}),
            "upload_id is required",
        ),
    ];
    for (value, needle) in cases {
        let error = attach(&t, &ctx, value).await.expect_err(needle);
        assert_eq!(code(&error), "invalid_params");
        assert!(error.message.contains(needle), "{}", error.message);
    }
    let without = tools(&r, false);
    let error = attach(
        &without,
        &ctx,
        json!({"action": "declare", "report_id": id, "name": "a", "mime": "a/b",
               "size": 1, "sha256": "0".repeat(64)}),
    )
    .await
    .expect_err("no uploads");
    assert_eq!(code(&error), "uploads_unavailable");
    let unknown = attach(
        &t,
        &ctx,
        json!({"action": "attach", "report_id": IssueReportId::generate(),
               "upload_id": DocumentUploadId::generate()}),
    )
    .await
    .expect_err("unknown report");
    assert_eq!(code(&unknown), "not_found");
}
