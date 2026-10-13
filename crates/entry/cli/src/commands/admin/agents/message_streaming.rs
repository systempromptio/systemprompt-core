//! Streaming message handling for `admin agents message`.
//!
//! The agent answers `SendStreamingMessage` with server-sent events, each a
//! JSON-RPC response wrapping one A2A `StreamResponse`. The stream closing is
//! the terminal signal; the consumer never reconnects.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::{Context, Result, anyhow};
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use sse_stream::SseStream;
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcResponse, Request};
use systemprompt_agent::models::a2a::protocol::SendMessageParams;
use systemprompt_logging::CliService;
use systemprompt_models::a2a::{
    Message, PROTOCOL_VERSION, StreamResponse, Task, TaskArtifactUpdateEvent, TaskState,
    VERSION_HEADER,
};

use super::message::extract_text_from_parts;
use super::types::MessageOutput;

#[derive(Debug, Default)]
pub struct StreamOutcome {
    pub task: Option<Task>,
    pub text: String,
}

pub(super) async fn execute_streaming(
    agent: &str,
    agent_url: &str,
    auth_token: &str,
    request: &Request<SendMessageParams>,
    message_text: &str,
) -> Result<MessageOutput> {
    let response = Client::new()
        .post(agent_url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {auth_token}"))
        .header(VERSION_HEADER, PROTOCOL_VERSION)
        .json(request)
        .send()
        .await
        .with_context(|| format!("Failed to open stream to agent at {agent_url}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.with_context(|| {
            format!("Agent stream request failed with status {status}; its body was unreadable")
        })?;
        anyhow::bail!("Agent stream request failed with status {status}: {body}");
    }

    let outcome = collect_stream(response.bytes_stream(), &mut write_stdout_chunk).await?;
    CliService::output("");

    let response = if outcome.text.is_empty() {
        outcome
            .task
            .as_ref()
            .and_then(|task| task.status.message.as_ref())
            .map(|msg| extract_text_from_parts(&msg.parts))
    } else {
        Some(outcome.text)
    };

    Ok(MessageOutput {
        agent: agent.to_owned(),
        task: outcome.task,
        message_sent: message_text.to_owned(),
        response,
    })
}

pub async fn collect_stream<S, E>(
    bytes: S,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<StreamOutcome>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Send,
    E: std::error::Error + Send + Sync + 'static,
{
    let mut events = std::pin::pin!(SseStream::from_bytes_stream(bytes));
    let mut outcome = StreamOutcome::default();

    while let Some(event) = events.next().await {
        let sse = event.map_err(|e| anyhow!("SSE stream error: {e}"))?;
        let Some(data) = sse.data else {
            continue;
        };
        apply_frame(&data, &mut outcome, on_text)?;
    }

    match outcome.task.as_ref() {
        Some(task) if !stream_may_end_in(task.status.state) => anyhow::bail!(
            "Stream closed while task {} was still {}",
            task.id,
            task.status.state.as_str()
        ),
        None if outcome.text.is_empty() => {
            anyhow::bail!("Stream closed without a task or message")
        },
        _ => Ok(outcome),
    }
}

const fn stream_may_end_in(state: TaskState) -> bool {
    state.is_terminal() || matches!(state, TaskState::InputRequired | TaskState::AuthRequired)
}

fn apply_frame(
    data: &str,
    outcome: &mut StreamOutcome,
    on_text: &mut (dyn FnMut(&str) + Send),
) -> Result<()> {
    let response: JsonRpcResponse<StreamResponse> =
        serde_json::from_str(data).context("Agent sent an undecodable stream frame")?;

    if let Some(error) = response.error {
        let details = error
            .data
            .map_or_else(String::new, |d| format!("\n\nDetails: {d}"));
        anyhow::bail!(
            "Agent returned error ({}): {}{}",
            error.code,
            error.message,
            details
        );
    }

    match response.result {
        Some(StreamResponse::Task(task)) => outcome.task = Some(task),
        Some(StreamResponse::Message(message)) => emit_text(&message, outcome, on_text),
        Some(StreamResponse::StatusUpdate(update)) => {
            if let Some(message) = &update.status.message {
                emit_text(message, outcome, on_text);
            }
            match outcome.task.as_mut() {
                Some(task) => task.status = update.status,
                None => {
                    outcome.task = Some(Task {
                        id: update.task_id,
                        context_id: update.context_id,
                        status: update.status,
                        ..Task::default()
                    });
                },
            }
        },
        Some(StreamResponse::ArtifactUpdate(update)) => apply_artifact(update, outcome),
        None => {},
    }
    Ok(())
}

fn emit_text(
    message: &Message,
    outcome: &mut StreamOutcome,
    on_text: &mut (dyn FnMut(&str) + Send),
) {
    let text = extract_text_from_parts(&message.parts);
    if !text.is_empty() {
        on_text(&text);
        outcome.text.push_str(&text);
    }
}

fn apply_artifact(update: TaskArtifactUpdateEvent, outcome: &mut StreamOutcome) {
    let task = outcome.task.get_or_insert_with(|| Task {
        id: update.task_id.clone(),
        context_id: update.context_id.clone(),
        ..Task::default()
    });
    let artifacts = task.artifacts.get_or_insert_with(Vec::new);
    let existing = artifacts
        .iter_mut()
        .find(|artifact| artifact.id == update.artifact.id);
    match existing {
        Some(artifact) if update.append => artifact.parts.extend(update.artifact.parts),
        Some(artifact) => *artifact = update.artifact,
        None => artifacts.push(update.artifact),
    }
}

fn write_stdout_chunk(text: &str) {
    if let Err(e) = std::io::Write::write_all(&mut std::io::stdout(), text.as_bytes()) {
        tracing::warn!(error = %e, "stdout write failed");
    }
    if let Err(e) = std::io::Write::flush(&mut std::io::stdout()) {
        tracing::warn!(error = %e, "stdout flush failed");
    }
}
