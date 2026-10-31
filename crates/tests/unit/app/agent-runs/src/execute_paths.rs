use std::time::Duration;

use systemprompt_agent_runs::execute::{ExecuteParams, RunReport, execute, execute_with_host};
use systemprompt_agent_runs::{
    AgentRunsError, AgentRunsResult, ErrorClass, ExitInfo, HarnessRef, HeadlessHost,
    HostCapabilities, HostEvent, HostInvocation, HostKind, PrepareContext, RunEventPayload,
    RunLayout, RunOutcome, RunRequest, RunStatus, RunnerLevel,
};
use systemprompt_identifiers::ResourceRevisionId;
use systemprompt_models::managed::RevisionBundle;
use tokio_util::sync::CancellationToken;

use crate::execute_support::{CollectSink, ShHost, credentials, request};
use crate::harness::{bundle, harness_ref};

struct StdinHost(ShHost);

impl HeadlessHost for StdinHost {
    fn kind(&self) -> HostKind {
        self.0.kind()
    }

    fn capabilities(&self) -> HostCapabilities {
        self.0.capabilities()
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        let mut invocation = self.0.prepare(ctx)?;
        invocation.stdin = Some("fed through stdin\n".to_owned());
        Ok(invocation)
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        self.0.parse_line(line)
    }

    fn outcome(&self, exit: ExitInfo, events: &[RunEventPayload], req: &RunRequest) -> RunOutcome {
        self.0.outcome(exit, events, req)
    }
}

struct Ran {
    result: AgentRunsResult<RunReport>,
    sink: CollectSink,
    layout: RunLayout,
    _dir: tempfile::TempDir,
}

async fn run(
    host: Option<&dyn HeadlessHost>,
    kind: HostKind,
    harness: Option<(&HarnessRef, &RevisionBundle)>,
) -> Ran {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));
    layout.create().await.unwrap();
    let mut req = request(Duration::from_secs(30));
    req.host = kind;
    req.workspace = layout.workspace.clone();
    let creds = credentials();
    let sink = CollectSink::default();
    let params = ExecuteParams {
        request: &req,
        layout: &layout,
        credentials: &creds,
        gateway_base_url: "http://127.0.0.1:1",
        hooks_file: None,
        seq_after: None,
        sink: &sink,
        cancel: CancellationToken::new(),
        harness,
    };
    let result = match host {
        Some(host) => execute_with_host(host, params).await,
        None => execute(params).await,
    };
    Ran {
        result,
        sink,
        layout,
        _dir: dir,
    }
}

fn sh(script: &str) -> ShHost {
    ShHost {
        script: script.to_owned(),
        keep: &[],
    }
}

fn messages(sink: &CollectSink, wanted: RunnerLevel) -> Vec<String> {
    sink.events()
        .into_iter()
        .filter_map(|event| match event.payload {
            RunEventPayload::Runner { message, level } if level == wanted => Some(message),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_desktop_host_is_unsupported_and_its_home_is_removed() {
    let ran = run(None, HostKind::ClaudeDesktop, None).await;

    match ran.result {
        Err(AgentRunsError::UnsupportedHost { host, reason }) => {
            assert_eq!(host, HostKind::ClaudeDesktop);
            assert!(reason.contains("do not run headless"), "{reason}");
        },
        other => panic!("expected unsupported host, got {other:?}"),
    }
    assert!(!ran.layout.home.exists());
}

#[tokio::test]
async fn a_verified_harness_is_installed_into_the_workspace_before_the_run() {
    let bundle = bundle();
    let harness = harness_ref(&bundle);
    let host = sh("cat AGENTS.md");

    let ran = run(Some(&host), HostKind::ClaudeCode, Some((&harness, &bundle))).await;

    let report = ran.result.expect("run");
    assert_eq!(report.outcome.status, RunStatus::Succeeded);
    assert_eq!(messages(&ran.sink, RunnerLevel::Info), vec!["# harness"]);
}

#[tokio::test]
async fn a_harness_for_another_root_refuses_the_run_without_spawning() {
    let bundle = bundle();
    let mut harness = harness_ref(&bundle);
    harness.revision_id = ResourceRevisionId::new("rev-other");
    let host = sh("echo spawned");

    let ran = run(Some(&host), HostKind::ClaudeCode, Some((&harness, &bundle))).await;

    let report = ran.result.expect("refusal is a report");
    assert_eq!(report.outcome.status, RunStatus::Failed);
    assert_eq!(report.outcome.error_class, ErrorClass::HarnessUnavailable);
    let error = report.outcome.error.expect("error message");
    assert!(error.starts_with("harness root is rev-root"), "{error}");
    assert!(messages(&ran.sink, RunnerLevel::Info).is_empty());
    assert_eq!(messages(&ran.sink, RunnerLevel::Error), vec![error]);
    assert!(!ran.layout.workspace.join("AGENTS.md").exists());
}

#[tokio::test]
async fn stdin_is_fed_to_the_host() {
    let host = StdinHost(sh("cat"));

    let ran = run(Some(&host), HostKind::ClaudeCode, None).await;

    assert_eq!(
        ran.result.expect("run").outcome.status,
        RunStatus::Succeeded
    );
    assert_eq!(
        messages(&ran.sink, RunnerLevel::Info),
        vec!["fed through stdin"]
    );
}

#[tokio::test]
async fn a_failed_run_without_an_outcome_error_carries_its_stderr() {
    let host = sh("echo broken >&2; exit 3");

    let ran = run(Some(&host), HostKind::ClaudeCode, None).await;

    let report = ran.result.expect("run");
    assert_eq!(report.outcome.status, RunStatus::Failed);
    assert_eq!(report.outcome.exit_code, Some(3));
    assert_eq!(report.outcome.error.as_deref(), Some("broken"));
}
