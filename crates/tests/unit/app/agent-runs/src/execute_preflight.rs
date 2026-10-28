//! Executor client version probe: pre-spawn refusals and version parsing.

use std::time::Duration;

use systemprompt_agent_runs::execute::version::{VersionError, first_version};
use systemprompt_agent_runs::execute::{ExecuteParams, RunReport, error_chain, execute_with_host};
use systemprompt_agent_runs::{
    AgentRunsResult, ErrorClass, ExitInfo, HarnessRef, HeadlessHost, HostCapabilities, HostEvent,
    HostInvocation, HostKind, PrepareContext, RunEventPayload, RunLayout, RunOutcome, RunRequest,
    RunStatus, RunnerLevel, VersionRequirement,
};
use systemprompt_models::managed::RevisionBundle;
use tokio_util::sync::CancellationToken;

use crate::execute_support::{CollectSink, credentials, request};

struct ProbeHost {
    program: &'static str,
    probe: &'static [&'static str],
    minimum: &'static str,
    stdin: Option<&'static str>,
    script: &'static str,
}

impl HeadlessHost for ProbeHost {
    fn kind(&self) -> HostKind {
        HostKind::ClaudeCode
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities::default()
    }

    fn version_requirement(&self) -> Option<VersionRequirement> {
        (!self.probe.is_empty()).then(|| VersionRequirement {
            args: self.probe,
            minimum: semver::Version::parse(self.minimum).unwrap(),
        })
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        Ok(HostInvocation {
            program: self.program.to_owned(),
            args: vec!["-c".to_owned(), self.script.to_owned()],
            env: Vec::new(),
            cwd: ctx.layout.workspace.clone(),
            stdin: self.stdin.map(str::to_owned),
        })
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        vec![HostEvent {
            payloads: vec![RunEventPayload::Runner {
                message: line.to_owned(),
                level: RunnerLevel::Info,
            }],
            session_id: None,
        }]
    }

    fn outcome(&self, exit: ExitInfo, _: &[RunEventPayload], _: &RunRequest) -> RunOutcome {
        let succeeded = exit.code == Some(0);
        RunOutcome {
            status: if succeeded {
                RunStatus::Succeeded
            } else {
                RunStatus::Failed
            },
            error_class: if succeeded {
                ErrorClass::Ok
            } else {
                ErrorClass::HostCrashed
            },
            session_id: None,
            cost_microdollars: 0,
            input_tokens: 0,
            output_tokens: 0,
            turns: 0,
            duration_ms: exit.duration_ms,
            structured_output: None,
            exit_code: exit.code,
            error: None,
            final_text: None,
        }
    }
}

fn host(probe: &'static [&'static str], minimum: &'static str) -> ProbeHost {
    ProbeHost {
        program: "/bin/sh",
        probe,
        minimum,
        stdin: None,
        script: "echo ran",
    }
}

async fn drive(
    host: &ProbeHost,
    req: &RunRequest,
    harness: Option<(&HarnessRef, &RevisionBundle)>,
) -> (
    AgentRunsResult<RunReport>,
    CollectSink,
    RunLayout,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));
    layout.create().await.unwrap();
    let sink = CollectSink::default();
    let creds = credentials();
    let result = execute_with_host(
        host,
        ExecuteParams {
            request: req,
            layout: &layout,
            credentials: &creds,
            gateway_base_url: "http://127.0.0.1:1",
            hooks_file: None,
            seq_after: Some(4),
            sink: &sink,
            cancel: CancellationToken::new(),
            harness,
        },
    )
    .await;
    (result, sink, layout, dir)
}

fn refusal_lines(sink: &CollectSink) -> Vec<(i64, String)> {
    sink.events()
        .into_iter()
        .filter_map(|e| match e.payload {
            RunEventPayload::Runner {
                message,
                level: RunnerLevel::Error,
            } => Some((e.seq, message)),
            _ => None,
        })
        .collect()
}

fn assert_refused(report: &RunReport, class: ErrorClass, needle: &str) {
    assert_eq!(report.outcome.status, RunStatus::Failed);
    assert_eq!(report.outcome.error_class, class);
    assert_eq!(report.outcome.exit_code, None);
    assert!(report.diff.entries.is_empty());
    assert!(report.stderr_tail.is_empty());
    let error = report.outcome.error.as_deref().unwrap();
    assert!(error.contains(needle), "{error}");
}

#[tokio::test]
async fn new_enough_client_runs() {
    let h = host(&["-c", "echo tool v2.4.1 build"], "2.0.0");
    let (result, sink, layout, _dir) = drive(&h, &request(Duration::from_secs(30)), None).await;
    let report = result.unwrap();
    assert_eq!(report.outcome.status, RunStatus::Succeeded);
    assert!(refusal_lines(&sink).is_empty());
    assert!(!layout.home.exists());
}

#[tokio::test]
async fn version_on_stderr_counts() {
    let h = host(&["-c", "echo client/3.0.0 >&2"], "3.0.0");
    let (result, _, _, _dir) = drive(&h, &request(Duration::from_secs(30)), None).await;
    assert_eq!(result.unwrap().outcome.status, RunStatus::Succeeded);
}

#[tokio::test]
async fn old_client_is_refused_before_spawn() {
    let h = ProbeHost {
        script: "echo spawned > spawned.txt",
        ..host(&["-c", "echo tool 1.9.9"], "2.0.0")
    };
    let (result, sink, layout, _dir) = drive(&h, &request(Duration::from_secs(30)), None).await;
    let report = result.unwrap();
    assert_refused(
        &report,
        ErrorClass::ConnectorUnavailable,
        "1.9.9 is below the required 2.0.0",
    );
    assert!(!layout.workspace.join("spawned.txt").exists());
    let lines = refusal_lines(&sink);
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].0, 5);
    assert_eq!(Some(lines[0].1.as_str()), report.outcome.error.as_deref());
    assert!(!layout.home.exists());
}

#[tokio::test]
async fn client_without_version_is_refused() {
    let h = host(&["-c", "echo no version here"], "1.0.0");
    let (result, _, _, _dir) = drive(&h, &request(Duration::from_secs(30)), None).await;
    assert_refused(
        &result.unwrap(),
        ErrorClass::ConnectorUnavailable,
        "printed no version; requires >= 1.0.0",
    );
}

#[tokio::test]
async fn missing_client_binary_is_refused_with_its_cause() {
    let h = ProbeHost {
        program: "/nonexistent/agent-cli",
        ..host(&["--version"], "1.0.0")
    };
    let (result, _, _, _dir) = drive(&h, &request(Duration::from_secs(30)), None).await;
    let report = result.unwrap();
    assert_refused(
        &report,
        ErrorClass::ConnectorUnavailable,
        "/nonexistent/agent-cli version probe failed: ",
    );
}

#[test]
fn error_chain_joins_every_source() {
    let error = VersionError::Probe {
        program: "cli".to_owned(),
        source: std::io::Error::new(std::io::ErrorKind::NotFound, "no such file"),
    };
    assert_eq!(
        error_chain(&error),
        "cli version probe failed: no such file"
    );
    let flat = VersionError::TimedOut {
        program: "cli".to_owned(),
    };
    assert_eq!(error_chain(&flat), "cli version probe timed out");
}

#[test]
fn first_version_reads_the_first_semver_token() {
    assert_eq!(
        first_version("codex-cli 0.46.0\n"),
        Some(semver::Version::new(0, 46, 0))
    );
    assert_eq!(
        first_version("tool/v1.2.3 node/20.1.0"),
        Some(semver::Version::new(1, 2, 3))
    );
    assert_eq!(first_version("1.2 and 4"), None);
}
