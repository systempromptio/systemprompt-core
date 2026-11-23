#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::sync::Arc;

use systemprompt_files::issues::IssueFiling;
use systemprompt_identifiers::{IssueReportId, UserId};
use systemprompt_issues::commands::{TriageArgs, triage_as};
use systemprompt_models::issues::{IssueContext, IssueKind, IssueSeverity, IssueStatus};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context, test_database_url};

async fn app() -> Arc<AppContext> {
    ensure_test_bootstrap();
    let url = test_database_url();
    test_app_context(&url).await
}

async fn filed(app: &Arc<AppContext>) -> IssueReportId {
    let (id, filed) = systemprompt_test_fixtures::documents_handles(&app)
        .issues
        .file(IssueFiling {
            reporter: UserId::new(format!("cli-triage-{}", uuid::Uuid::new_v4())),
            via_agent: None,
            kind: IssueKind::Bug,
            severity: IssueSeverity::Medium,
            title: format!("triage {}", uuid::Uuid::new_v4()),
            body: "steps".to_owned(),
            fingerprint_key: None,
            context: IssueContext::default(),
            client: serde_json::Value::Null,
        })
        .await
        .unwrap();
    assert!(filed.inserted);
    id
}

fn args(id: &str, status: IssueStatus, note: Option<&str>) -> TriageArgs {
    TriageArgs {
        id: id.to_owned(),
        status,
        note: note.map(ToOwned::to_owned),
    }
}

#[tokio::test]
async fn triage_moves_a_report_and_records_the_note() {
    let app = app().await;
    let id = filed(&app).await;
    let triager = UserId::new(format!("cli-triager-{}", uuid::Uuid::new_v4()));

    let output = triage_as(
        args(id.as_str(), IssueStatus::NeedsInfo, Some("which browser?")),
        &triager,
        &systemprompt_test_fixtures::documents_handles(&app).issues,
    )
    .await
    .unwrap();
    let card = serde_json::to_value(output.artifact()).unwrap().to_string();
    assert!(card.contains("needs_info"), "{card}");
    assert!(card.contains("which browser?"), "{card}");

    let report = systemprompt_test_fixtures::documents_handles(&app)
        .issues
        .find(&id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(report.status, IssueStatus::NeedsInfo);

    let again = triage_as(
        args(id.as_str(), IssueStatus::Resolved, None),
        &triager,
        &systemprompt_test_fixtures::documents_handles(&app).issues,
    )
    .await
    .unwrap();
    let card = serde_json::to_value(again.artifact()).unwrap().to_string();
    assert!(
        card.contains("needs_info") && card.contains("resolved"),
        "{card}"
    );
}

#[tokio::test]
async fn triage_refuses_a_malformed_id_before_querying() {
    let app = app().await;
    let err = triage_as(
        args("not-a-uuid", IssueStatus::Closed, None),
        &UserId::new("cli-triager"),
        &systemprompt_test_fixtures::documents_handles(&app).issues,
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "invalid issue report id: not-a-uuid");
}

#[tokio::test]
async fn triage_of_an_unknown_report_fails() {
    let app = app().await;
    let missing = uuid::Uuid::new_v4().to_string();
    let err = triage_as(
        args(&missing, IssueStatus::Closed, None),
        &UserId::new("cli-triager"),
        &systemprompt_test_fixtures::documents_handles(&app).issues,
    )
    .await
    .unwrap_err();
    let issue = err
        .downcast_ref::<systemprompt_files::issues::IssuesError>()
        .expect("issues error");
    assert!(
        matches!(issue, systemprompt_files::issues::IssuesError::NotFound(id) if id.as_str() == missing),
        "{issue:?}"
    );
}
