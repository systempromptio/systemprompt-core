//! The `/bin/sh` fake host, the collecting sink and the run helper the
//! executor tests drive.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_agent_runs::execute::{ExecuteParams, RunReport, execute_with_host};
use systemprompt_agent_runs::{
    ErrorClass, EventSink, ExitInfo, HeadlessHost, AgentRunsResult, HostCapabilities, HostEvent,
    HostInvocation, HostKind, PermissionMode, PrepareContext, RunCredentials, RunEvent,
    RunEventPayload, RunLayout, RunOutcome, RunRequest, RunStatus, RunnerLevel,
};
use systemprompt_identifiers::{ApiKeyId, ApiKeySecret, HostSessionId, SessionId, UserId};
use tokio_util::sync::CancellationToken;

pub(crate) struct ShHost {
    pub(crate) script: String,
    pub(crate) keep: &'static [&'static str],
}

impl HeadlessHost for ShHost {
    fn kind(&self) -> HostKind {
        HostKind::ClaudeCode
    }

    fn persistent_home_paths(&self) -> &'static [&'static str] {
        self.keep
    }

    fn capabilities(&self) -> HostCapabilities {
        HostCapabilities::default()
    }

    fn prepare(&self, ctx: &PrepareContext<'_>) -> AgentRunsResult<HostInvocation> {
        Ok(HostInvocation {
            program: "/bin/sh".to_owned(),
            args: vec!["-c".to_owned(), self.script.clone()],
            env: vec![("RUN_MARK".to_owned(), "1".to_owned())],
            cwd: ctx.layout.workspace.clone(),
            stdin: None,
        })
    }

    fn parse_line(&self, line: &str) -> Vec<HostEvent> {
        if let Some(id) = line.strip_prefix("INIT ") {
            let session_id = HostSessionId::try_new(id).unwrap();
            return vec![HostEvent {
                payloads: vec![RunEventPayload::Init {
                    session_id: session_id.clone(),
                    model: "sh".to_owned(),
                    tools: Vec::new(),
                    cwd: None,
                }],
                session_id: Some(session_id),
            }];
        }
        vec![HostEvent {
            payloads: vec![RunEventPayload::Runner {
                message: line.to_owned(),
                level: RunnerLevel::Info,
            }],
            session_id: None,
        }]
    }

    fn outcome(&self, exit: ExitInfo, _: &[RunEventPayload], _: &RunRequest) -> RunOutcome {
        let (status, error_class) = if exit.cancelled {
            (RunStatus::Cancelled, ErrorClass::Ok)
        } else if exit.timed_out {
            (RunStatus::TimedOut, ErrorClass::Timeout)
        } else if exit.code == Some(0) {
            (RunStatus::Succeeded, ErrorClass::Ok)
        } else {
            (RunStatus::Failed, ErrorClass::HostCrashed)
        };
        RunOutcome {
            status,
            error_class,
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

#[derive(Default)]
pub(crate) struct CollectSink {
    pub(crate) batches: Mutex<Vec<Vec<RunEvent>>>,
    pub(crate) sessions: Mutex<Vec<HostSessionId>>,
}

#[async_trait]
impl EventSink for CollectSink {
    async fn emit(&self, batch: Vec<RunEvent>) -> AgentRunsResult<()> {
        self.batches.lock().unwrap().push(batch);
        Ok(())
    }

    async fn session_observed(&self, session_id: &HostSessionId) -> AgentRunsResult<()> {
        self.sessions.lock().unwrap().push(session_id.clone());
        Ok(())
    }
}

impl CollectSink {
    pub(crate) fn events(&self) -> Vec<RunEvent> {
        self.batches
            .lock()
            .unwrap()
            .iter()
            .flatten()
            .cloned()
            .collect()
    }
}

fn request(wall_clock: Duration) -> RunRequest {
    RunRequest {
        host: HostKind::ClaudeCode,
        prompt: "p".to_owned(),
        system_prompt_append: None,
        persona: None,
        model: None,
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        permission_mode: PermissionMode::Default,
        permission_prompts_none: false,
        workspace: PathBuf::from("."),
        add_dirs: Vec::new(),
        mcp_servers: Default::default(),
        output_schema: None,
        max_turns: None,
        max_budget_usd: None,
        wall_clock,
        resume_session_id: None,
        attempt: 1,
        attribution_scopes: Vec::new(),
        plugin_id: None,
        plugin_dirs: Vec::new(),
    }
}

fn credentials() -> RunCredentials {
    RunCredentials {
        user_id: UserId::new("user-1"),
        api_key_id: ApiKeyId::new("key-1"),
        pat: ApiKeySecret::new("secret"),
        gateway_session_id: SessionId::new("sess-1"),
        hook_token: None,
        hook_token_jti: None,
        hook_token_expires_at: None,
    }
}

pub(crate) struct Run {
    pub(crate) report: RunReport,
    pub(crate) sink: CollectSink,
    pub(crate) layout: RunLayout,
    _dir: tempfile::TempDir,
}

pub(crate) async fn run_script(
    script: &str,
    wall_clock: Duration,
    cancel: CancellationToken,
) -> Run {
    run_with(script, wall_clock, cancel, None).await
}

pub(crate) async fn run_with(
    script: &str,
    wall_clock: Duration,
    cancel: CancellationToken,
    seq_after: Option<i64>,
) -> Run {
    let dir = tempfile::tempdir().unwrap();
    let layout = RunLayout::new(dir.path().join("run"));
    layout.create().await.unwrap();
    std::fs::write(layout.home.join("secret.txt"), "x").unwrap();
    let host = ShHost {
        script: script.to_owned(),
        keep: &[],
    };
    let sink = CollectSink::default();
    let req = request(wall_clock);
    let creds = credentials();
    let report = execute_with_host(
        &host,
        ExecuteParams {
            request: &req,
            layout: &layout,
            credentials: &creds,
            gateway_base_url: "http://127.0.0.1:1",
            hooks_file: None,
            seq_after,
            sink: &sink,
            cancel,
        },
    )
    .await
    .unwrap();
    Run {
        report,
        sink,
        layout,
        _dir: dir,
    }
}


pub(crate) async fn run_in(layout: &RunLayout, host: &ShHost) -> RunReport {
    layout.create().await.unwrap();
    let sink = CollectSink::default();
    let req = request(Duration::from_secs(30));
    let creds = credentials();
    execute_with_host(
        host,
        ExecuteParams {
            request: &req,
            layout,
            credentials: &creds,
            gateway_base_url: "http://127.0.0.1:1",
            hooks_file: None,
            seq_after: None,
            sink: &sink,
            cancel: CancellationToken::new(),
        },
    )
    .await
    .unwrap()
}
