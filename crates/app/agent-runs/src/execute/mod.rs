//! Runs one prepared request to its [`RunReport`].
//!
//! [`execute`] resolves the host, snapshots the working tree, spawns the host
//! as an owned subprocess in its own process group, pumps its stdout and
//! stderr into the [`EventSink`], supervises the wall clock and the
//! cancellation token, classifies the exit through the adapter and diffs the
//! working tree. The run's `home/` is removed on every path, error or not.
//!
//! [`ExecuteParams::seq_after`] is the last `seq` already persisted for a
//! resumed run; the first event is numbered one past it, or 0.
//! The adapter's persistent home paths survive between attempts
//! (`home_keep`). [`execute_with_host`] takes an explicit adapter. The child
//! inherits only [`INHERITED_ENV`] plus the invocation's own env.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod command;
pub mod diff;
pub mod harness;
mod home_keep;
pub mod outputs;
mod pump;
pub mod sink;
mod spawn;
mod supervise;
pub mod version;

use std::path::PathBuf;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

pub use command::INHERITED_ENV;
pub use diff::{DiffEntry, DiffKind, DiffSummary};
pub use sink::EventSink;

use crate::credentials::RunCredentials;
use crate::error::{AgentRunsError, AgentRunsResult};
use crate::event::{RunEventPayload, RunnerLevel};
use crate::host::{Execution, ExitInfo, HeadlessHost, HostInvocation, PrepareContext, host_for};
use crate::layout::RunLayout;
use crate::outcome::{ErrorClass, RunOutcome, RunStatus};
use crate::request::{HarnessRef, RunRequest};
use systemprompt_models::managed::RevisionBundle;

const DRAIN_GRACE: Duration = Duration::from_secs(5);

/// Everything one execution needs; the caller (the claim loop) owns the
/// credentials, the hooks file and the sink.
pub struct ExecuteParams<'a> {
    pub request: &'a RunRequest,
    pub layout: &'a RunLayout,
    pub credentials: &'a RunCredentials,
    pub gateway_base_url: &'a str,
    pub hooks_file: Option<PathBuf>,
    pub seq_after: Option<i64>,
    pub sink: &'a dyn EventSink,
    pub cancel: CancellationToken,
    pub harness: Option<(&'a HarnessRef, &'a RevisionBundle)>,
}

impl std::fmt::Debug for ExecuteParams<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecuteParams")
            .field("host", &self.request.host)
            .field("root", &self.layout.root)
            .field("seq_after", &self.seq_after)
            .finish_non_exhaustive()
    }
}

/// The classified outcome plus what the executor observed around it.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    pub outcome: RunOutcome,
    pub diff: DiffSummary,
    pub outputs: outputs::OutputCapture,
    pub stderr_tail: Vec<String>,
}

pub async fn execute(params: ExecuteParams<'_>) -> AgentRunsResult<RunReport> {
    match host_for(params.request.host) {
        Execution::Unsupported { reason } => {
            let result = Err(AgentRunsError::UnsupportedHost {
                host: params.request.host,
                reason,
            });
            finish(params.layout, result).await
        },
        Execution::Supported(host) => execute_with_host(host.as_ref(), params).await,
    }
}

pub async fn execute_with_host(
    host: &dyn HeadlessHost,
    params: ExecuteParams<'_>,
) -> AgentRunsResult<RunReport> {
    let layout = params.layout;
    let result = run(host, &params).await;
    let kept = home_keep::preserve(layout, host.persistent_home_paths()).await;
    let result = match (result, kept) {
        (Ok(_), Err(error)) => Err(error),
        (result, _) => result,
    };
    finish(layout, result).await
}

async fn finish(
    layout: &RunLayout,
    result: AgentRunsResult<RunReport>,
) -> AgentRunsResult<RunReport> {
    let removed = layout.remove_home().await;
    let report = result?;
    removed?;
    Ok(report)
}

async fn run(host: &dyn HeadlessHost, params: &ExecuteParams<'_>) -> AgentRunsResult<RunReport> {
    let ctx = PrepareContext {
        request: params.request,
        layout: params.layout,
        credentials: params.credentials,
        gateway_base_url: params.gateway_base_url,
        hooks_file: params.hooks_file.clone(),
    };
    if let Some((harness_ref, bundle)) = params.harness {
        let root = harness::target_root(host.harness_targets(), params.layout, params.request);
        if let Err(error) = harness::install(harness_ref.clone(), bundle.clone(), root).await {
            let message = error_chain(&error);
            return Ok(refused(params, ErrorClass::HarnessUnavailable, message).await);
        }
    }
    let invocation = host.prepare(&ctx)?;
    if let Some(requirement) = host.version_requirement()
        && let Err(error) = version::check(&requirement, &invocation).await
    {
        let message = error_chain(&error);
        return Ok(refused(params, ErrorClass::ConnectorUnavailable, message).await);
    }
    home_keep::restore(params.layout, host.persistent_home_paths()).await?;
    let before = diff::snapshot_async(invocation.cwd.clone()).await;

    let mut sink = pump::BufferedSink::new(params.sink, params.seq_after);
    let (stderr_tail, exit) = drive(host, params, &invocation, &mut sink).await?;
    let shown = stderr_tail.len().saturating_sub(pump::STDERR_EVENT_LINES);
    for line in &stderr_tail[shown..] {
        sink.push(RunEventPayload::Stderr { line: line.clone() })
            .await;
    }
    if exit.cancelled || exit.timed_out {
        let message = if exit.cancelled {
            "run cancelled"
        } else {
            "wall clock exceeded"
        };
        sink.push(RunEventPayload::Runner {
            message: message.to_owned(),
            level: RunnerLevel::Warn,
        })
        .await;
    }
    sink.flush().await;
    let payloads = sink.finish()?;

    let mut outcome = host.outcome(exit, &payloads, params.request);
    if outcome.status != RunStatus::Succeeded && outcome.error.is_none() && !stderr_tail.is_empty()
    {
        outcome.error = Some(stderr_tail.join("\n"));
    }
    let after = diff::snapshot_async(invocation.cwd.clone()).await;
    let diff = diff::workspace_diff(&before, &after);
    let outputs = outputs::capture(&invocation.cwd, &diff, params.layout).await?;
    Ok(RunReport {
        outcome,
        diff,
        outputs,
        stderr_tail,
    })
}

#[must_use]
pub fn error_chain(error: &dyn std::error::Error) -> String {
    let mut parts = vec![error.to_string()];
    let mut cause = error.source();
    while let Some(next) = cause {
        parts.push(next.to_string());
        cause = next.source();
    }
    parts.join(": ")
}

async fn refused(
    params: &ExecuteParams<'_>,
    error_class: ErrorClass,
    message: String,
) -> RunReport {
    let mut sink = pump::BufferedSink::new(params.sink, params.seq_after);
    sink.push(RunEventPayload::Runner {
        message: message.clone(),
        level: RunnerLevel::Error,
    })
    .await;
    sink.flush().await;
    RunReport {
        outcome: RunOutcome {
            status: RunStatus::Failed,
            error_class,
            session_id: None,
            cost_microdollars: 0,
            input_tokens: 0,
            output_tokens: 0,
            turns: 0,
            duration_ms: 0,
            structured_output: None,
            exit_code: None,
            error: Some(message),
            final_text: None,
        },
        diff: DiffSummary::default(),
        outputs: outputs::OutputCapture::default(),
        stderr_tail: Vec::new(),
    }
}

async fn drive(
    host: &dyn HeadlessHost,
    params: &ExecuteParams<'_>,
    invocation: &HostInvocation,
    sink: &mut pump::BufferedSink<'_>,
) -> AgentRunsResult<(Vec<String>, ExitInfo)> {
    let started = Instant::now();
    let mut owned = spawn::spawn_owned_supervised(invocation)?;
    let stdout = owned.child.stdout.take();
    let stderr = owned.child.stderr.take();
    let stdin = owned.child.stdin.take();

    let stop = CancellationToken::new();
    let pumps_done = CancellationToken::new();
    let stdout_log = params.layout.logs.join("stdout.jsonl");
    let stderr_log = params.layout.logs.join("stderr.log");

    let pumps = async {
        let feed = async {
            if let (Some(mut pipe), Some(input)) = (stdin, invocation.stdin.as_deref()) {
                if let Err(error) = pipe.write_all(input.as_bytes()).await {
                    tracing::warn!(%error, "headless stdin write failed");
                }
                drop(pipe);
            }
        };
        let out = async {
            if let Some(stdout) = stdout {
                pump::pump_stdout(stdout, &stdout_log, host, sink, &stop).await;
            }
        };
        let err = async {
            match stderr {
                Some(stderr) => pump::pump_stderr(stderr, &stderr_log, &stop).await,
                None => Vec::new(),
            }
        };
        let ((), (), tail) = tokio::join!(feed, out, err);
        pumps_done.cancel();
        tail
    };
    let watch = async {
        let exit = supervise::supervise(
            &mut owned,
            params.request.wall_clock,
            &params.cancel,
            started,
        )
        .await;
        tokio::select! {
            () = tokio::time::sleep(DRAIN_GRACE) => stop.cancel(),
            () = pumps_done.cancelled() => {},
        }
        exit
    };
    let (stderr_tail, exit) = tokio::join!(pumps, watch);
    drop(owned);

    Ok((stderr_tail, exit))
}
