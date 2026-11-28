use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use systemprompt_harness::command::{
    Answers, ControlCommand, EventWriter, ExitStatus, PermissionMode, RunReport, RunRequest, run,
    run_controlled,
};
use systemprompt_traits::CancellationToken;

use crate::command::support::host;

fn request(workspace: &Path) -> RunRequest {
    RunRequest {
        prompts: vec!["hello".to_owned()],
        workspace: workspace.to_path_buf(),
        permission_mode: PermissionMode::Ask,
        answers: Answers::parse(r#"{"permissions":[{"type":"once"}]}"#).unwrap(),
        model: None,
        agent: None,
        data_root: None,
        sync_timeout: Duration::from_secs(5),
        timeout: None,
    }
}

fn lines(bytes: Vec<u8>) -> Vec<Value> {
    String::from_utf8(bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[tokio::test]
async fn a_run_with_no_account_ends_not_signed_in_with_one_finished_line() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    let mut out = EventWriter::new(Vec::new());

    let report = run(
        &host,
        request(dir.path()),
        &CancellationToken::new(),
        &mut out,
    )
    .await;

    assert_eq!(
        report,
        RunReport {
            status: ExitStatus::NotSignedIn,
            session_id: None,
            message: Some("no account is signed in; run login first".to_owned()),
        }
    );
    assert_eq!(
        lines(out.into_inner()),
        [json!({
            "type": "finished",
            "session_id": null,
            "outcome": "not_signed_in",
            "exit_code": 5,
            "message": "no account is signed in; run login first",
            "unused_answers": { "permissions": 1, "questions": 0 },
        })]
    );
}

#[tokio::test]
async fn a_run_opens_its_store_and_accounts_under_the_data_root_it_is_given() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    let chosen = dir.path().join("chosen");
    let mut out = EventWriter::new(Vec::new());
    let request = RunRequest {
        data_root: Some(chosen.clone()),
        ..request(dir.path())
    };

    let report = run(&host, request, &CancellationToken::new(), &mut out).await;

    assert_eq!(report.status, ExitStatus::NotSignedIn);
    assert!(chosen.join("data").join("sessions.db").is_file());
    assert!(chosen.join("state").join("accounts.json").is_file());
    assert!(!host.data_root.exists());
}

#[tokio::test]
async fn a_data_root_that_cannot_be_created_is_an_internal_failure() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file, not a directory").unwrap();
    let mut out = EventWriter::new(Vec::new());
    let request = RunRequest {
        data_root: Some(blocker.join("root")),
        ..request(dir.path())
    };

    let report = run(&host, request, &CancellationToken::new(), &mut out).await;

    assert_eq!(report.status, ExitStatus::Internal);
    assert_eq!(report.session_id, None);
    let written = lines(out.into_inner());
    assert_eq!(written.len(), 1);
    assert_eq!(written[0]["type"], "finished");
    assert_eq!(written[0]["outcome"], "internal");
    assert_eq!(written[0]["exit_code"], 70);
}

#[tokio::test]
async fn a_controlled_run_that_cannot_start_ends_without_reading_its_controls() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    let (sender, controls) = tokio::sync::mpsc::unbounded_channel();
    sender.send(Ok(ControlCommand::Abort {})).unwrap();
    let mut out = EventWriter::new(Vec::new());

    let report = run_controlled(
        &host,
        request(dir.path()),
        (&CancellationToken::new(), controls),
        &mut out,
    )
    .await;

    assert_eq!(report.status, ExitStatus::NotSignedIn);
    assert_eq!(lines(out.into_inner()).len(), 1);
}
