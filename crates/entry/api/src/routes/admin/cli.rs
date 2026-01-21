use axum::extract::Extension;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::time::Duration;
use systemprompt_models::api::{ApiError, CliExecuteRequest, CliOutputEvent};
use systemprompt_models::auth::UserType;
use systemprompt_models::RequestContext;
use systemprompt_runtime::AppContext;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

const MAX_TIMEOUT_SECS: u64 = 600;
const CLI_BINARY_PATH: &str = "/app/bin/systemprompt";

pub fn router() -> Router<AppContext> {
    Router::new().route("/", post(execute_cli))
}

async fn execute_cli(
    Extension(req_ctx): Extension<RequestContext>,
    Json(request): Json<CliExecuteRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if req_ctx.user_type() != UserType::Admin {
        return Err(ApiError::forbidden("Admin role required for CLI gateway"));
    }

    let args = request.args;
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
        session_id: req_ctx.session_id().to_string(),
        context_id,
        user_id: req_ctx.user_id().to_string(),
        auth_token: if auth_token.is_empty() {
            None
        } else {
            Some(auth_token.to_string())
        },
    };

    let stream = create_cli_stream(args, timeout, session_env);

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

struct SessionEnv {
    session_id: String,
    context_id: String,
    user_id: String,
    auth_token: Option<String>,
}

fn create_cli_stream(
    args: Vec<String>,
    timeout: Duration,
    session_env: SessionEnv,
) -> impl Stream<Item = Result<Event, Infallible>> {
    async_stream::stream! {
        let mut cmd = Command::new(CLI_BINARY_PATH);
        cmd.args(&args)
            .env("SYSTEMPROMPT_CLI_REMOTE", "1")
            .env("SYSTEMPROMPT_SESSION_ID", &session_env.session_id)
            .env("SYSTEMPROMPT_CONTEXT_ID", &session_env.context_id)
            .env("SYSTEMPROMPT_USER_ID", &session_env.user_id)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(token) = &session_env.auth_token {
            cmd.env("SYSTEMPROMPT_AUTH_TOKEN", token);
        }

        let mut child = match cmd.spawn()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "Failed to spawn CLI process");
                yield Ok(CliOutputEvent::Error { message: e.to_string() }.to_sse_event());
                yield Ok(CliOutputEvent::ExitCode { code: 1 }.to_sse_event());
                return;
            }
        };

        let pid = child.id().unwrap_or(0);
        yield Ok(CliOutputEvent::Started { pid }.to_sse_event());

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<CliOutputEvent>(100);

        if let Some(stdout) = stdout {
            let tx = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx
                        .send(CliOutputEvent::Stdout {
                            data: format!("{}\n", line),
                        })
                        .await;
                }
            });
        }

        if let Some(stderr) = stderr {
            let tx = tx.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx
                        .send(CliOutputEvent::Stderr {
                            data: format!("{}\n", line),
                        })
                        .await;
                }
            });
        }

        drop(tx);

        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Some(e) => yield Ok(e.to_sse_event()),
                        None => break,
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    tracing::warn!(timeout_secs = timeout.as_secs(), "CLI command timed out");
                    let _ = child.kill().await;
                    yield Ok(CliOutputEvent::Error {
                        message: format!("Timeout after {}s", timeout.as_secs())
                    }.to_sse_event());
                    yield Ok(CliOutputEvent::ExitCode { code: -1 }.to_sse_event());
                    return;
                }
            }
        }

        match child.wait().await {
            Ok(status) => {
                let code = status.code().unwrap_or(-1);
                tracing::info!(exit_code = code, "CLI command completed");
                yield Ok(CliOutputEvent::ExitCode { code }.to_sse_event());
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to wait for CLI process");
                yield Ok(CliOutputEvent::Error { message: e.to_string() }.to_sse_event());
                yield Ok(CliOutputEvent::ExitCode { code: -1 }.to_sse_event());
            }
        }
    }
}
