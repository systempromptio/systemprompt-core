//! Remote CLI execution over a deployment's SSE gateway.
//!
//! [`RemoteCliExecutor`] POSTs a [`CliExecuteRequest`] to
//! `/api/v1/admin/cli` and streams the resulting `cli` server-sent events
//! back through a caller-supplied [`OutputSink`], returning the remote
//! process's exit code. Transport failures mid-stream are reported through
//! the sink and surface as exit code `1` rather than an error, matching an
//! interactive terminal session; only setup failures return [`ClientError`].

use std::io;
use std::time::Duration;

use futures::StreamExt;
use reqwest_eventsource::{Event, EventSource};
use systemprompt_models::api::{CliExecuteRequest, CliOutputEvent};

use crate::error::{ClientError, ClientResult};

pub trait OutputSink: Send {
    fn stdout_chunk(&mut self, data: &str) -> io::Result<()>;
    fn stderr_chunk(&mut self, data: &str) -> io::Result<()>;
    fn error_message(&mut self, message: &str);
}

#[derive(Debug, Clone, Copy)]
pub struct RemoteCliRequest<'a> {
    pub token: &'a str,
    pub context: &'a str,
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
            context_id: if request.context.is_empty() {
                None
            } else {
                Some(systemprompt_identifiers::ContextId::new(request.context))
            },
        };

        let mut builder = self
            .client
            .post(&self.execute_url)
            .header("Authorization", format!("Bearer {}", request.token))
            .header("Accept", "text/event-stream");

        if !request.context.is_empty() {
            builder = builder.header("x-context-id", request.context);
        }

        stream_response(builder.json(&body), sink).await
    }
}

async fn stream_response(
    builder: reqwest::RequestBuilder,
    sink: &mut dyn OutputSink,
) -> ClientResult<i32> {
    let mut es = EventSource::new(builder).map_err(|_e| ClientError::EventStreamSetup)?;
    let mut exit_code = 0;

    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Message(msg)) if msg.event == "cli" => {
                match serde_json::from_str::<CliOutputEvent>(&msg.data) {
                    Ok(evt) => {
                        exit_code = dispatch_event(evt, sink, exit_code)?;
                    },
                    Err(e) => {
                        tracing::warn!(error = %e, data = %msg.data, "Failed to parse CLI event");
                    },
                }
            },
            Ok(Event::Open | Event::Message(_)) => {},
            Err(reqwest_eventsource::Error::StreamEnded) => break,
            Err(e) => {
                sink.error_message(&format!("Connection error: {e}"));
                return Ok(1);
            },
        }
    }

    Ok(exit_code)
}

fn dispatch_event(
    event: CliOutputEvent,
    sink: &mut dyn OutputSink,
    current_exit_code: i32,
) -> ClientResult<i32> {
    match event {
        CliOutputEvent::Stdout { data } => {
            sink.stdout_chunk(&data)?;
            Ok(current_exit_code)
        },
        CliOutputEvent::Stderr { data } => {
            sink.stderr_chunk(&data)?;
            Ok(current_exit_code)
        },
        CliOutputEvent::ExitCode { code } => Ok(code),
        CliOutputEvent::Error { message } => {
            sink.error_message(&message);
            Ok(current_exit_code)
        },
        CliOutputEvent::Started { pid } => {
            tracing::debug!(pid = pid, "Remote process started");
            Ok(current_exit_code)
        },
    }
}
