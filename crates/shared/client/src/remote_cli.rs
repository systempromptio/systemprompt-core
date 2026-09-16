//! Remote CLI execution over a deployment's SSE gateway.
//!
//! [`RemoteCliExecutor`] POSTs a [`CliExecuteRequest`] to
//! `/api/v1/admin/cli` and streams the resulting `cli` server-sent events
//! back through a caller-supplied [`OutputSink`], returning the remote
//! process's exit code. A transport failure mid-stream is reported through
//! the sink and surfaces as exit code `1`, matching an interactive terminal
//! session. A stream that ends before the server has sent an `ExitCode`
//! event — a server or proxy that died mid-command — is
//! [`ClientError::ServerUnavailable`], never a success; so is a `cli` event
//! the client cannot decode, since it may have been the exit code.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::time::Duration;

use futures::StreamExt;
use sse_stream::{Sse, SseStream};
use systemprompt_identifiers::{ContextId, SessionToken};
use systemprompt_models::api::{CliExecuteRequest, CliOutputEvent};

use crate::error::{ClientError, ClientResult};

const CLI_EVENT: &str = "cli";

pub trait OutputSink: Send {
    fn stdout_chunk(&mut self, data: &str) -> io::Result<()>;
    fn stderr_chunk(&mut self, data: &str) -> io::Result<()>;
    fn error_message(&mut self, message: &str);
}

#[derive(Debug, Clone, Copy)]
pub struct RemoteCliRequest<'a> {
    pub token: &'a SessionToken,
    pub context: Option<&'a ContextId>,
    pub args: &'a [String],
}

#[derive(Debug, Clone)]
pub struct RemoteCliExecutor {
    client: reqwest::Client,
    execute_url: String,
    timeout_secs: u64,
}

impl RemoteCliExecutor {
    pub fn new(base_url: &str, timeout_secs: u64) -> ClientResult<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs + 30))
            .build()?;
        Ok(Self {
            client,
            execute_url: format!("{base_url}/api/v1/admin/cli"),
            timeout_secs,
        })
    }

    pub async fn execute(
        &self,
        request: RemoteCliRequest<'_>,
        sink: &mut dyn OutputSink,
    ) -> ClientResult<i32> {
        let body = CliExecuteRequest {
            args: request.args.to_vec(),
            timeout_secs: self.timeout_secs,
            context_id: request.context.cloned(),
        };

        let mut builder = self
            .client
            .post(&self.execute_url)
            .header(
                "Authorization",
                format!("Bearer {}", request.token.as_str()),
            )
            .header("Accept", "text/event-stream");

        if let Some(context) = request.context {
            builder = builder.header("x-context-id", context.as_str());
        }

        let response = builder.json(&body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await?;
            return Err(ClientError::from_response(status.as_u16(), text));
        }

        stream_response(response, sink).await
    }
}

async fn stream_response(
    response: reqwest::Response,
    sink: &mut dyn OutputSink,
) -> ClientResult<i32> {
    let mut events = SseStream::from_bytes_stream(response.bytes_stream());
    let mut exit_code: Option<i32> = None;

    while let Some(event) = events.next().await {
        match event {
            Ok(sse) => {
                if let Some(code) = handle_event(sse, sink)? {
                    exit_code = Some(code);
                }
            },
            Err(e) => {
                sink.error_message(&format!("Connection error: {e}"));
                return Ok(1);
            },
        }
    }

    exit_code.ok_or_else(|| {
        ClientError::ServerUnavailable("stream ended before the remote exit code".to_owned())
    })
}

fn handle_event(sse: Sse, sink: &mut dyn OutputSink) -> ClientResult<Option<i32>> {
    if sse.event.as_deref() != Some(CLI_EVENT) {
        return Ok(None);
    }
    let Some(data) = sse.data else {
        return Ok(None);
    };
    let event = serde_json::from_str::<CliOutputEvent>(&data).map_err(|e| {
        ClientError::ServerUnavailable(format!("undecodable cli event from server: {e}"))
    })?;
    dispatch_event(event, sink)
}

fn dispatch_event(event: CliOutputEvent, sink: &mut dyn OutputSink) -> ClientResult<Option<i32>> {
    match event {
        CliOutputEvent::Stdout { data } => {
            sink.stdout_chunk(&data)?;
            Ok(None)
        },
        CliOutputEvent::Stderr { data } => {
            sink.stderr_chunk(&data)?;
            Ok(None)
        },
        CliOutputEvent::ExitCode { code } => Ok(Some(code)),
        CliOutputEvent::Error { message } => {
            sink.error_message(&message);
            Ok(None)
        },
        CliOutputEvent::Started { pid } => {
            tracing::debug!(pid = pid, "Remote process started");
            Ok(None)
        },
    }
}
