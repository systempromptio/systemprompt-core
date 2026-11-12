//! A resumed attempt resumes only when the host's kept session store is
//! present; otherwise it starts a fresh session with the original prompt.

use std::time::Duration;

use systemprompt_agent_runs::execute::{ExecuteParams, RunReport, execute_with_host};
use systemprompt_agent_runs::{
    AgentRunsResult, ExitInfo, HeadlessHost, HostCapabilities, HostEvent, HostInvocation, HostKind,
    PrepareContext, RunEventPayload, RunLayout, RunOutcome, RunRequest, RunnerLevel,
};
use systemprompt_identifiers::HostSessionId;
use tokio_util::sync::CancellationToken;

use crate::execute_support::{CollectSink, ShHost, credentials, request};

struct ResumeEcho(ShHost);

impl HeadlessHost for ResumeEcho {
    fn kind(&self) -> HostKind {
        self.0.kind()
    }

    fn capabilities(&self) -> HostCapabilities {
        self.0.capabilities()
    }

    fn persistent_home_paths(&self) -> &'static [&'static str] {
        self.0.keep
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        let mut invocation = self.0.prepare(ctx)?;
        let resume = ctx
            .request
            .resume_session_id
            .as_ref()
            .map_or("none", |session| session.as_str());
        invocation.args = vec![
            "-c".to_owned(),
            format!("echo resume={resume} prompt={}", ctx.request.prompt),
        ];
        Ok(invocation)
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        self.0.parse_line(line)
    }

    fn outcome(&self, exit: ExitInfo, events: &[RunEventPayload], req: &RunRequest) -> RunOutcome {
        self.0.outcome(exit, events, req)
    }
}

const KEEP: &[&str] = &["keep/me"];

async fn attempt(layout: &RunLayout, keep: &'static [&'static str]) -> (RunReport, Vec<String>) {
    layout.create().await.unwrap();
    let host = ResumeEcho(ShHost {
        script: String::new(),
        keep,
    });
    let mut req = request(Duration::from_secs(30));
    req.prompt = "original-input".to_owned();
    req.attempt = 2;
    req.resume_session_id = Some(HostSessionId::try_new("sess-prev").unwrap());
    let creds = credentials();
    let sink = CollectSink::default();
    let report = execute_with_host(
        &host,
        ExecuteParams {
            request: &req,
            layout,
            credentials: &creds,
            gateway_base_url: "http://127.0.0.1:1",
            hooks_file: None,
            seq_after: None,
            sink: &sink,
            cancel: CancellationToken::new(),
            harness: None,
        },
    )
    .await
    .unwrap();
    let lines = sink
        .events()
        .into_iter()
        .filter_map(|event| match event.payload {
            RunEventPayload::Runner {
                message,
                level: RunnerLevel::Info,
            } => Some(message),
            _ => None,
        })
        .collect();
    (report, lines)
}

#[tokio::test]
async fn a_missing_session_store_starts_a_fresh_session_with_the_original_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));

    let (_, lines) = attempt(&layout, KEEP).await;

    assert_eq!(lines, vec!["resume=none prompt=original-input"]);
}

#[tokio::test]
async fn a_restored_session_store_keeps_the_resume() {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));
    let kept = layout.host.join("home-keep/keep/me");
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::write(kept.join("session.txt"), "s1").unwrap();

    let (_, lines) = attempt(&layout, KEEP).await;

    assert_eq!(lines, vec!["resume=sess-prev prompt=original-input"]);
}

#[tokio::test]
async fn a_host_that_keeps_nothing_keeps_the_resume() {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));

    let (_, lines) = attempt(&layout, &[]).await;

    assert_eq!(lines, vec!["resume=sess-prev prompt=original-input"]);
}
