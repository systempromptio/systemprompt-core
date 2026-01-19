//! Remote CLI execution via SSE streaming.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest_eventsource::{Event, EventSource};
use std::io::{self, Write};
use systemprompt_core_logging::CliService;
use systemprompt_models::api::{CliExecuteRequest, CliOutputEvent};

pub async fn execute_remote(
    hostname: &str,
    token: &str,
    context_id: &str,
    args: &[String],
    timeout_secs: u64,
) -> Result<i32> {
    let url = format!("https://{}/api/v1/admin/cli", hostname);
    let request = CliExecuteRequest {
        args: args.to_vec(),
        timeout_secs,
        context_id: if context_id.is_empty() {
            None
        } else {
            Some(context_id.to_string())
        },
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs + 30))
        .build()
        .context("Failed to create HTTP client")?;

    let mut request_builder = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "text/event-stream");

    if !context_id.is_empty() {
        request_builder = request_builder.header("x-context-id", context_id);
    }

    let request_builder = request_builder.json(&request);

    stream_response(request_builder).await
}

async fn stream_response(request_builder: reqwest::RequestBuilder) -> Result<i32> {
    let mut es = EventSource::new(request_builder).context("Failed to create event stream")?;
    let mut exit_code = 0;
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();

    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Message(msg)) if msg.event == "cli" => {
                match serde_json::from_str::<CliOutputEvent>(&msg.data) {
                    Ok(evt) => {
                        exit_code = handle_cli_event(evt, &mut stdout, &mut stderr, exit_code)?;
                    },
                    Err(e) => {
                        tracing::warn!(error = %e, data = %msg.data, "Failed to parse CLI event");
                    },
                }
            },
            Ok(Event::Open | Event::Message(_)) => {},
            Err(reqwest_eventsource::Error::StreamEnded) => break,
            Err(e) => {
                CliService::error(&format!("Connection error: {e}"));
                return Ok(1);
            },
        }
    }

    Ok(exit_code)
}

fn handle_cli_event(
    event: CliOutputEvent,
    stdout: &mut io::Stdout,
    stderr: &mut io::Stderr,
    current_exit_code: i32,
) -> Result<i32> {
    match event {
        CliOutputEvent::Stdout { data } => {
            write!(stdout, "{}", data).context("Failed to write to stdout")?;
            stdout.flush().context("Failed to flush stdout")?;
            Ok(current_exit_code)
        },
        CliOutputEvent::Stderr { data } => {
            write!(stderr, "{}", data).context("Failed to write to stderr")?;
            stderr.flush().context("Failed to flush stderr")?;
            Ok(current_exit_code)
        },
        CliOutputEvent::ExitCode { code } => Ok(code),
        CliOutputEvent::Error { message } => {
            CliService::error(&message);
            Ok(current_exit_code)
        },
        CliOutputEvent::Started { pid } => {
            tracing::debug!(pid = pid, "Remote process started");
            Ok(current_exit_code)
        },
    }
}
