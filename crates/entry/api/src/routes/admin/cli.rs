//! Admin CLI gateway route: streams `systemprompt` subprocess output over SSE.
//!
//! Exposes a single authenticated endpoint that validates and forwards an argv
//! to the CLI binary, propagating the caller's session/context/auth into the
//! child's environment and relaying stdout/stderr as [`CliOutputEvent`] frames.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Extension;
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use systemprompt_events::ToSse;
use systemprompt_models::RequestContext;
use systemprompt_models::api::{ApiError, CliExecuteRequest, CliOutputEvent};
use systemprompt_runtime::AppContext;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

fn cli_event_to_sse(event: &CliOutputEvent) -> Event {
    event
        .to_sse()
        .unwrap_or_else(|_| Event::default().event("cli").data("{}"))
}

const MAX_TIMEOUT_SECS: u64 = 600;
const DEFAULT_CLI_BINARY_PATH: &str = "/app/bin/systemprompt";
const MAX_CLI_ARGS: usize = 32;

/// Resolved path to the `systemprompt` CLI binary the gateway forwards argv to.
///
/// Injected as a router extension so deployments (and tests) can point the
/// gateway at a specific binary instead of the baked-in default.
#[derive(Clone)]
pub(crate) struct CliBinaryPath(Arc<str>);

impl CliBinaryPath {
    pub(crate) fn new(path: impl AsRef<str>) -> Self {
        Self(Arc::from(path.as_ref()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CliBinaryPath {
    fn default() -> Self {
        Self::new(DEFAULT_CLI_BINARY_PATH)
    }
}
const MAX_CLI_ARG_LEN: usize = 256;

// The CLI subprocess is spawned without a shell, so this is defence-in-depth
// against argv smuggling (flag injection via crafted `--foo=$(...)` payloads,
// NUL-byte truncation) reaching downstream tooling that does invoke a shell.
fn validate_cli_args(args: &[String]) -> Result<(), Box<ApiError>> {
    if args.is_empty() {
        return Err(Box::new(ApiError::bad_request(
            "cli args must not be empty",
        )));
    }
    if args.len() > MAX_CLI_ARGS {
        return Err(Box::new(ApiError::bad_request(format!(
            "too many cli args (max {MAX_CLI_ARGS})"
        ))));
    }
    for (i, arg) in args.iter().enumerate() {
        if arg.len() > MAX_CLI_ARG_LEN {
            return Err(Box::new(ApiError::bad_request(format!(
                "cli arg #{i} exceeds {MAX_CLI_ARG_LEN} bytes"
            ))));
        }
        if arg
            .chars()
            .any(|c| c.is_control() || matches!(c, '`' | '$' | '|' | ';' | '&' | '\n' | '\r'))
        {
            return Err(Box::new(ApiError::bad_request(format!(
                "cli arg #{i} contains forbidden character"
            ))));
        }
    }
    let first = &args[0];
    let first_ok = first.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && first
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !first_ok {
        return Err(Box::new(ApiError::bad_request(
            "first cli arg must be a lowercase subcommand",
        )));
    }
    Ok(())
}

pub(super) fn router() -> Router<AppContext> {
    router_with_binary(CliBinaryPath::default())
}

fn router_with_binary(binary: CliBinaryPath) -> Router<AppContext> {
    Router::new()
        .route("/", post(execute_cli))
        .layer(Extension(binary))
}

async fn execute_cli(
    Extension(req_ctx): Extension<RequestContext>,
    Extension(binary): Extension<CliBinaryPath>,
    Json(request): Json<CliExecuteRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let args = request.args;
    validate_cli_args(&args).map_err(|e| *e)?;
    let timeout_secs = request.timeout_secs.min(MAX_TIMEOUT_SECS);
    let timeout = Duration::from_secs(timeout_secs);

    tracing::info!(
        user_id = %req_ctx.user_id(),
        args = ?args,
        timeout_secs = timeout_secs,
        "CLI gateway: executing command"
    );

    let auth_token = req_ctx.auth_token().as_str();
    let context_id = req_ctx.context_id().to_string();
    let session_env = SessionEnv {
        session: req_ctx.session_id().to_string(),
        context: context_id,
        user: req_ctx.user_id().to_string(),
        auth_token: if auth_token.is_empty() {
            None
        } else {
            Some(auth_token.to_owned())
        },
    };

    let stream = create_cli_stream(binary, args, timeout, session_env);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

struct SessionEnv {
    session: String,
    context: String,
    user: String,
    auth_token: Option<String>,
}

fn build_cli_command(binary: &CliBinaryPath, args: &[String], session_env: &SessionEnv) -> Command {
    let mut cmd = Command::new(binary.as_str());
    cmd.args(args)
        .env("SYSTEMPROMPT_CLI_REMOTE", "1")
        .env("SYSTEMPROMPT_SESSION_ID", &session_env.session)
        .env("SYSTEMPROMPT_CONTEXT_ID", &session_env.context)
        .env("SYSTEMPROMPT_USER_ID", &session_env.user)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    if let Some(token) = &session_env.auth_token {
        cmd.env("SYSTEMPROMPT_AUTH_TOKEN", token);
    }
    cmd
}

fn spawn_line_forwarder<R>(
    reader: R,
    tx: tokio::sync::mpsc::Sender<CliOutputEvent>,
    make_event: fn(String) -> CliOutputEvent,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(make_event(format!("{line}\n"))).await.is_err() {
                break;
            }
        }
    });
}

async fn kill_on_timeout(
    child: &mut tokio::process::Child,
    timeout: Duration,
) -> [CliOutputEvent; 2] {
    tracing::warn!(timeout_secs = timeout.as_secs(), "CLI command timed out");
    if let Err(e) = child.kill().await {
        tracing::error!(error = %e, "Failed to kill CLI process");
    }
    [
        CliOutputEvent::Error {
            message: format!("Timeout after {}s", timeout.as_secs()),
        },
        CliOutputEvent::ExitCode { code: -1 },
    ]
}

async fn wait_exit_events(mut child: tokio::process::Child) -> Vec<CliOutputEvent> {
    match child.wait().await {
        Ok(status) => {
            let code = status.code().unwrap_or_else(|| {
                tracing::debug!("Process terminated by signal");
                -1
            });
            tracing::info!(exit_code = code, "CLI command completed");
            vec![CliOutputEvent::ExitCode { code }]
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to wait for CLI process");
            vec![
                CliOutputEvent::Error {
                    message: e.to_string(),
                },
                CliOutputEvent::ExitCode { code: -1 },
            ]
        },
    }
}

fn create_cli_stream(
    binary: CliBinaryPath,
    args: Vec<String>,
    timeout: Duration,
    session_env: SessionEnv,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut child = match build_cli_command(&binary, &args, &session_env).spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "Failed to spawn CLI process");
                yield Ok(cli_event_to_sse(&CliOutputEvent::Error { message: e.to_string() }));
                yield Ok(cli_event_to_sse(&CliOutputEvent::ExitCode { code: 1 }));
                return;
            }
        };

        let pid = child.id().unwrap_or_else(|| {
            tracing::debug!("Child process has no PID (already exited?)");
            0
        });
        yield Ok(cli_event_to_sse(&CliOutputEvent::Started { pid }));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<CliOutputEvent>(100);
        if let Some(stdout) = child.stdout.take() {
            spawn_line_forwarder(stdout, tx.clone(), |data| CliOutputEvent::Stdout { data });
        }
        if let Some(stderr) = child.stderr.take() {
            spawn_line_forwarder(stderr, tx.clone(), |data| CliOutputEvent::Stderr { data });
        }
        drop(tx);

        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(ref e) => yield Ok(cli_event_to_sse(e)),
                        None => break,
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    for event in kill_on_timeout(&mut child, timeout).await {
                        yield Ok(cli_event_to_sse(&event));
                    }
                    return;
                }
            }
        }

        for event in wait_exit_events(child).await {
            yield Ok(cli_event_to_sse(&event));
        }
    }
}

/// Test-only seam: build the CLI gateway router pointed at an arbitrary binary.
///
/// Pointing it at (e.g.) `/bin/sh` wrapping a fixture script lets the
/// subprocess forward, timeout, and exit-code paths be exercised without the
/// deployed binary.
#[cfg(feature = "test-api")]
pub mod test_api {
    use super::{CliBinaryPath, router_with_binary};
    use axum::Router;
    use systemprompt_runtime::AppContext;

    pub fn cli_router_with_binary(path: &str) -> Router<AppContext> {
        router_with_binary(CliBinaryPath::new(path))
    }
}
