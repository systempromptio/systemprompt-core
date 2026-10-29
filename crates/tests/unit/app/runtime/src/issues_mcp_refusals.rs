use systemprompt_files::issues::IssuesError;
use systemprompt_identifiers::{DocumentUploadId, IssueReportId};
use systemprompt_runtime::issues::mcp::refusal::{refusal_result, tool_error};

fn code(error: IssuesError) -> (String, String) {
    let mcp = tool_error(error);
    let result = refusal_result(&mcp).expect("a refusal is a tool result");
    assert_eq!(result.is_error, Some(true));
    let data = result.structured_content.expect("structured refusal");
    (
        data["code"].as_str().expect("code").to_owned(),
        mcp.message.to_string(),
    )
}

#[test]
fn each_refusal_carries_its_code() {
    let report = IssueReportId::generate();
    let upload = DocumentUploadId::generate();
    let cases = [
        (IssuesError::Disabled, "disabled"),
        (
            IssuesError::Invalid {
                field: "title",
                reason: "must not be empty".to_owned(),
            },
            "invalid_params",
        ),
        (IssuesError::RateLimited { limit: 3 }, "rate_limited"),
        (IssuesError::NotFound(report.clone()), "not_found"),
        (IssuesError::NotActive(report.clone()), "not_active"),
        (
            IssuesError::AttachmentsFull {
                report: report.clone(),
                limit: 5,
            },
            "attachments_full",
        ),
        (IssuesError::UploadNotReady(upload), "upload_not_ready"),
    ];
    for (error, expected) in cases {
        assert_eq!(code(error).0, expected);
    }
}

#[test]
fn a_refusal_message_names_what_was_refused() {
    let report = IssueReportId::generate();
    let (_, message) = code(IssuesError::NotFound(report.clone()));
    assert!(message.contains(report.as_str()), "{message}");
    let (_, message) = code(IssuesError::RateLimited { limit: 7 });
    assert!(message.contains('7'), "{message}");
}

#[test]
fn a_database_failure_is_an_internal_error_with_a_fixed_message() {
    let mcp = tool_error(IssuesError::Database(sqlx::Error::PoolTimedOut));
    assert!(refusal_result(&mcp).is_none(), "not a coded refusal");
    assert_eq!(mcp.message, "the issue store failed; try the call again");
    assert!(mcp.data.is_none());
}
