use std::io::Cursor;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use systemprompt_device::client::ManifestKey;
use systemprompt_device::secure::Zeroizing;
use systemprompt_device::view::KeyFingerprint;
use systemprompt_harness::command::{
    Answers, Composition, EventWriter, ExitStatus, HeadlessHost, LoginFailure, LoginOutcome,
    LoginReport, LoginRequest, PermissionMode, RunRequest, UsageError, login, read_token, run,
};
use systemprompt_traits::CancellationToken;

use crate::command::support::{FIRST_KEY, FakeInstance, PAT, SECOND_KEY, closed_port_url, host};

fn fingerprint(raw: [u8; 32]) -> KeyFingerprint {
    ManifestKey::from_raw(raw).fingerprint().clone()
}

fn request(url: &str, expected: Option<KeyFingerprint>) -> LoginRequest {
    LoginRequest {
        instance: url.to_owned(),
        token: Zeroizing::new(PAT.to_owned()),
        fingerprint: expected,
        data_root: None,
    }
}

fn failure(report: &LoginReport) -> LoginFailure {
    match &report.outcome {
        LoginOutcome::Failed { reason, .. } => *reason,
        LoginOutcome::SignedIn { .. } => panic!("expected a failed sign-in, got {report:?}"),
    }
}

fn needs_sign_in(host: &HeadlessHost) -> bool {
    let accounts = Composition::open_accounts(host, &host.root(None)).unwrap();
    let needed = accounts.snapshot().needs_sign_in();
    accounts.shutdown();
    needed
}

fn accounts_file(host: &HeadlessHost) -> String {
    std::fs::read_to_string(host.root(None).accounts_file()).unwrap()
}

fn run_request(workspace: &Path) -> RunRequest {
    RunRequest {
        prompts: vec!["hello".to_owned()],
        workspace: workspace.to_path_buf(),
        permission_mode: PermissionMode::Deny,
        answers: Answers::default(),
        model: None,
        agent: None,
        data_root: None,
        sync_timeout: Duration::from_secs(20),
        timeout: None,
    }
}

#[tokio::test]
async fn login_signs_in_pins_the_served_key_and_makes_the_account_active() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let report = login(&host, request(&instance.url, None)).await;

    assert_eq!(report.status, ExitStatus::Completed);
    let LoginOutcome::SignedIn {
        account,
        instance: url,
        user,
        key_fingerprint,
    } = &report.outcome
    else {
        panic!("expected a sign-in, got {report:?}");
    };
    assert_eq!(url, &instance.url);
    assert_eq!(user.as_deref(), Some("dev@example.test"));
    assert_eq!(key_fingerprint.as_ref(), Some(&fingerprint(FIRST_KEY)));

    let accounts = Composition::open_accounts(&host, &host.root(None)).unwrap();
    assert_eq!(accounts.active_id().as_ref(), Some(account));
    assert!(!accounts.snapshot().needs_sign_in());
    assert!(accounts.get(account).unwrap().trusted);
    assert_eq!(
        accounts.pinned_key(account).unwrap(),
        Some(ManifestKey::from_raw(FIRST_KEY))
    );
    accounts.shutdown();

    let stored = accounts_file(&host);
    assert!(!stored.contains(PAT));
    assert!(
        instance
            .seen()
            .contains(&"POST /v1/auth/bridge/pat".to_owned())
    );
}

#[tokio::test]
async fn the_login_line_names_the_account_the_instance_and_the_pinned_key() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();

    let report = login(&host(dir.path()), request(&instance.url, None)).await;

    let line: Value = serde_json::from_str(&report.encode().unwrap()).unwrap();
    let LoginOutcome::SignedIn { account, .. } = &report.outcome else {
        panic!("expected a sign-in, got {report:?}");
    };
    assert_eq!(
        line,
        json!({
            "type": "login",
            "exit_code": 0,
            "outcome": "signed_in",
            "account": account,
            "instance": instance.url,
            "user": "dev@example.test",
            "key_fingerprint": fingerprint(FIRST_KEY),
        })
    );
    assert!(!report.encode().unwrap().contains(PAT));
}

#[tokio::test]
async fn a_refused_token_fails_with_the_refused_status_and_stores_no_sign_in() {
    let instance = FakeInstance::start().await;
    instance.refuse_pat();
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let report = login(&host, request(&instance.url, None)).await;

    assert_eq!(report.status, ExitStatus::Refused);
    assert_eq!(failure(&report), LoginFailure::Refused);
    assert!(needs_sign_in(&host));
    assert_eq!(
        serde_json::from_str::<Value>(&report.encode().unwrap()).unwrap(),
        json!({
            "type": "login",
            "exit_code": 77,
            "outcome": "failed",
            "reason": "refused",
            "message": "the instance refused the token",
        })
    );
}

#[tokio::test]
async fn an_instance_that_does_not_answer_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let report = login(&host, request(&closed_port_url().await, None)).await;

    assert_eq!(report.status, ExitStatus::Unavailable);
    assert_eq!(failure(&report), LoginFailure::Unreachable);
    assert!(needs_sign_in(&host));
}

#[tokio::test]
async fn a_named_fingerprint_must_be_the_key_the_instance_serves() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let other = login(&host, request(&instance.url, Some(fingerprint(SECOND_KEY)))).await;
    assert_eq!(other.status, ExitStatus::Refused);
    assert_eq!(failure(&other), LoginFailure::KeyMismatch);
    assert!(needs_sign_in(&host));
    assert!(!accounts_file(&host).contains("\"pat\""));

    let named = login(&host, request(&instance.url, Some(fingerprint(FIRST_KEY)))).await;
    assert_eq!(named.status, ExitStatus::Completed);
    assert!(!needs_sign_in(&host));
}

#[tokio::test]
async fn a_key_that_replaces_the_pinned_one_is_accepted_only_by_name() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    assert_eq!(
        login(&host, request(&instance.url, None)).await.status,
        ExitStatus::Completed
    );

    instance.serve_second_key();
    let changed = login(&host, request(&instance.url, None)).await;
    assert_eq!(changed.status, ExitStatus::Refused);
    assert_eq!(failure(&changed), LoginFailure::KeyChanged);
    assert!(needs_sign_in(&host));

    let accepted = login(&host, request(&instance.url, Some(fingerprint(SECOND_KEY)))).await;
    assert_eq!(accepted.status, ExitStatus::Completed);
    let LoginOutcome::SignedIn {
        key_fingerprint, ..
    } = accepted.outcome
    else {
        panic!("expected a sign-in");
    };
    assert_eq!(key_fingerprint, Some(fingerprint(SECOND_KEY)));
}

#[tokio::test]
async fn a_second_login_with_the_same_key_keeps_it_pinned() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    let first = login(&host, request(&instance.url, None)).await;
    let second = login(&host, request(&instance.url, None)).await;
    assert_eq!(first, second);
    assert_eq!(second.status, ExitStatus::Completed);
}

#[tokio::test]
async fn a_run_after_login_reports_an_instance_that_serves_no_manifest_as_unavailable() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    assert_eq!(
        login(&host, request(&instance.url, None)).await.status,
        ExitStatus::Completed
    );
    let mut out = EventWriter::new(Vec::new());

    let report = run(
        &host,
        run_request(dir.path()),
        &CancellationToken::new(),
        &mut out,
    )
    .await;

    assert_eq!(report.status, ExitStatus::Unavailable);
    assert_eq!(report.session_id, None);
    assert!(
        instance
            .seen()
            .contains(&"GET /v1/bridge/manifest".to_owned())
    );
    let written = String::from_utf8(out.into_inner()).unwrap();
    let finished: Value = serde_json::from_str(written.trim_end()).unwrap();
    assert_eq!(finished["type"], "finished");
    assert_eq!(finished["exit_code"], 10);
}

#[test]
fn the_token_is_the_first_line_of_standard_input_trimmed() {
    let mut input = Cursor::new(format!("  {PAT} \r\nignored\n"));
    assert_eq!(read_token(&mut input).unwrap().as_str(), PAT);
}

#[test]
fn an_empty_standard_input_is_a_usage_error() {
    for text in ["", "\n", "   \n"] {
        let mut input = Cursor::new(text.to_owned());
        assert!(matches!(
            read_token(&mut input),
            Err(UsageError::EmptyToken)
        ));
    }
}
