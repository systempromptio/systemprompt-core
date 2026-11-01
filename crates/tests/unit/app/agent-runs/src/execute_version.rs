use systemprompt_agent_runs::execute::RunReport;
use systemprompt_agent_runs::execute::version::{VersionError, first_version};
use systemprompt_agent_runs::{
    AgentRunsResult, ErrorClass, ExitInfo, HeadlessHost, HostCapabilities, HostEvent,
    HostInvocation, HostKind, PrepareContext, RunEventPayload, RunLayout, RunOutcome, RunRequest,
    RunStatus, VersionRequirement,
};

use crate::execute_support::{ShHost, run_in};

struct VersionedHost {
    inner: ShHost,
    program: Option<&'static str>,
    probe: &'static [&'static str],
    minimum: &'static str,
}

impl HeadlessHost for VersionedHost {
    fn kind(&self) -> HostKind {
        self.inner.kind()
    }

    fn capabilities(&self) -> HostCapabilities {
        self.inner.capabilities()
    }

    fn version_requirement(&self) -> Option<VersionRequirement> {
        Some(VersionRequirement {
            args: self.probe,
            minimum: semver::Version::parse(self.minimum).unwrap(),
        })
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        let mut invocation = self.inner.prepare(ctx)?;
        if let Some(program) = self.program {
            program.clone_into(&mut invocation.program);
        }
        Ok(invocation)
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        self.inner.parse_line(line)
    }

    fn outcome(&self, exit: ExitInfo, events: &[RunEventPayload], req: &RunRequest) -> RunOutcome {
        self.inner.outcome(exit, events, req)
    }
}

async fn run(
    program: Option<&'static str>,
    probe: &'static [&'static str],
    minimum: &'static str,
) -> RunReport {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));
    let host = VersionedHost {
        inner: ShHost {
            script: "echo ran".to_owned(),
            keep: &[],
        },
        program,
        probe,
        minimum,
    };
    run_in(&layout, &host).await
}

fn assert_refused(report: &RunReport, expected: &str) {
    assert_eq!(report.outcome.status, RunStatus::Failed);
    assert_eq!(report.outcome.error_class, ErrorClass::ConnectorUnavailable);
    assert_eq!(report.outcome.error.as_deref(), Some(expected));
    assert_eq!(report.outcome.exit_code, None);
}

#[test]
fn first_version_reads_the_first_semver_token() {
    assert_eq!(
        first_version("claude 2.1.3 (Claude Code)"),
        Some(semver::Version::new(2, 1, 3))
    );
    assert_eq!(
        first_version("opencode/v0.15.2\nother 9.9.9"),
        Some(semver::Version::new(0, 15, 2))
    );
    assert_eq!(first_version("version 2.1 build"), None);
    assert_eq!(first_version(""), None);
}

#[tokio::test]
async fn a_new_enough_client_runs() {
    let report = run(None, &["-c", "echo tool v2.3.4"], "2.0.0").await;
    assert_eq!(report.outcome.status, RunStatus::Succeeded);
    assert_eq!(report.outcome.exit_code, Some(0));
}

#[tokio::test]
async fn a_version_on_stderr_counts() {
    let report = run(None, &["-c", "echo 3.0.0 >&2"], "3.0.0").await;
    assert_eq!(report.outcome.status, RunStatus::Succeeded);
}

#[tokio::test]
async fn an_old_client_is_refused_before_spawn() {
    let report = run(None, &["-c", "echo tool 1.9.9"], "2.0.0").await;
    assert_refused(&report, "/bin/sh 1.9.9 is below the required 2.0.0");
}

#[tokio::test]
async fn a_client_printing_no_version_is_refused() {
    let report = run(None, &["-c", "echo nothing here"], "2.0.0").await;
    assert_refused(&report, "/bin/sh printed no version; requires >= 2.0.0");
}

#[tokio::test]
async fn a_missing_program_fails_the_probe() {
    let report = run(
        Some("/nonexistent/headless-client"),
        &["--version"],
        "1.0.0",
    )
    .await;
    assert_eq!(report.outcome.error_class, ErrorClass::ConnectorUnavailable);
    let message = report.outcome.error.unwrap();
    assert!(
        message.starts_with("/nonexistent/headless-client version probe failed: "),
        "{message}"
    );
}

#[test]
fn version_errors_name_the_program() {
    let timed_out = VersionError::TimedOut {
        program: "claude".to_owned(),
    };
    assert_eq!(timed_out.to_string(), "claude version probe timed out");
}
