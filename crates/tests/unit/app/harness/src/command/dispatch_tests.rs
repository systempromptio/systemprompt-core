use std::io::Cursor;

use serde_json::Value;
use systemprompt_harness::command::{ExitStatus, HeadlessHost, Streams, USAGE, execute};
use systemprompt_traits::CancellationToken;

use crate::command::support::{FakeInstance, PAT, host};

struct Ended {
    status: ExitStatus,
    output: String,
    errors: String,
}

async fn invoke(host: &HeadlessHost, args: &[&str], input: &str) -> Ended {
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let status = execute(
        host,
        args.iter().map(|argument| (*argument).to_owned()).collect(),
        Streams {
            input: Cursor::new(input.to_owned()),
            output: &mut output,
            errors: &mut errors,
        },
        &CancellationToken::new(),
    )
    .await;
    Ended {
        status,
        output: String::from_utf8(output).unwrap(),
        errors: String::from_utf8(errors).unwrap(),
    }
}

#[tokio::test]
async fn a_usage_error_goes_to_the_error_stream_with_the_usage_text() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let ended = invoke(&host, &["serve"], "").await;

    assert_eq!(ended.status, ExitStatus::Usage);
    assert_eq!(ended.status.code(), 64);
    assert!(ended.output.is_empty());
    assert!(ended.errors.starts_with("unknown command `serve`"));
    assert!(ended.errors.contains(USAGE));
    assert!(!host.data_root.exists());
}

#[tokio::test]
async fn a_run_in_a_workspace_that_does_not_exist_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());
    let missing = dir.path().join("missing");

    let ended = invoke(
        &host,
        &[
            "run",
            "--workspace",
            missing.to_str().unwrap(),
            "--prompt",
            "hello",
        ],
        "",
    )
    .await;

    assert_eq!(ended.status, ExitStatus::Usage);
    assert!(ended.output.is_empty());
    assert!(ended.errors.contains("is not a directory"));
}

#[tokio::test]
async fn a_run_writes_its_event_stream_to_the_output() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let ended = invoke(
        &host,
        &[
            "run",
            "--workspace",
            dir.path().to_str().unwrap(),
            "--prompt",
            "hello",
        ],
        "",
    )
    .await;

    assert_eq!(ended.status, ExitStatus::NotSignedIn);
    assert!(ended.errors.is_empty());
    let line: Value = serde_json::from_str(ended.output.trim_end()).unwrap();
    assert_eq!(line["type"], "finished");
    assert_eq!(line["exit_code"], 5);
}

#[tokio::test]
async fn login_without_a_token_on_standard_input_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let ended = invoke(
        &host,
        &["login", "--stdin", "--instance", "http://127.0.0.1:9"],
        "\n",
    )
    .await;

    assert_eq!(ended.status, ExitStatus::Usage);
    assert!(ended.output.is_empty());
    assert!(
        ended
            .errors
            .starts_with("no token was read from standard input")
    );
}

#[tokio::test]
async fn login_reads_the_token_from_the_input_and_writes_one_line() {
    let instance = FakeInstance::start().await;
    let dir = tempfile::tempdir().unwrap();
    let host = host(dir.path());

    let ended = invoke(
        &host,
        &["login", "--stdin", "--instance", &instance.url],
        &format!("{PAT}\n"),
    )
    .await;

    assert_eq!(ended.status, ExitStatus::Completed);
    assert!(ended.errors.is_empty());
    assert_eq!(ended.output.lines().count(), 1);
    assert!(!ended.output.contains(PAT));
    let line: Value = serde_json::from_str(ended.output.trim_end()).unwrap();
    assert_eq!(line["type"], "login");
    assert_eq!(line["outcome"], "signed_in");
    assert_eq!(line["exit_code"], 0);
    assert_eq!(line["instance"], instance.url);
}
