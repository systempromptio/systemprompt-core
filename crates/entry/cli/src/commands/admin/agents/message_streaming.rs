use anyhow::{Result, anyhow};
use futures_util::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use systemprompt_agent::models::a2a::jsonrpc::{JsonRpcResponse, Request};
use systemprompt_agent::models::a2a::protocol::{MessageSendParams, TaskStatusUpdateEvent};
use systemprompt_logging::CliService;
use systemprompt_models::a2a::Task;

use super::message::extract_text_from_parts;
use super::types::MessageOutput;
use crate::shared::CommandResult;

pub async fn execute_streaming(
    agent: &str,
    agent_url: &str,
    auth_token: &str,
    request: &Request<MessageSendParams>,
    message_text: &str,
) -> Result<CommandResult<MessageOutput>> {
    let client = Client::new();
    let http_request = client
        .post(agent_url)
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(request);

    let mut es = EventSource::new(http_request)
        .map_err(|e| anyhow!("Failed to create SSE connection: {}", e))?;

    let mut final_task: Option<Task> = None;
    let mut accumulated_text = String::new();

    while let Some(event) = es.next().await {
        match event {
            Ok(Event::Open) => {
                tracing::debug!("SSE connection opened");
            },
            Ok(Event::Message(message)) => {
                match serde_json::from_str::<JsonRpcResponse<TaskStatusUpdateEvent>>(&message.data)
                {
                    Ok(response) => {
                        if let Some(error) = response.error {
                            let details = error
                                .data
                                .map_or_else(String::new, |d| format!("\n\nDetails: {}", d));
                            anyhow::bail!(
                                "Agent returned error ({}): {}{}",
                                error.code,
                                error.message,
                                details
                            );
                        }

                        if let Some(event) = response.result {
                            if let Some(ref msg) = event.status.message {
                                let text = extract_text_from_parts(&msg.parts);
                                if !text.is_empty() {
                                    let _ = std::io::Write::write_all(
                                        &mut std::io::stdout(),
                                        text.as_bytes(),
                                    );
                                    let _ = std::io::Write::flush(&mut std::io::stdout());
                                    accumulated_text.push_str(&text);
                                }
                            }

                            if event.is_final {
                                CliService::output("");
                                final_task = Some(Task {
                                    id: event.task_id.into(),
                                    context_id: event.context_id.into(),
                                    status: event.status,
                                    history: None,
                                    artifacts: None,
                                    metadata: None,
                                    created_at: None,
                                    last_modified: None,
                                });
                                break;
                            }
                        }
                    },
                    Err(e) => {
                        tracing::debug!(error = %e, data = %message.data, "Failed to parse SSE event");
                    },
                }
            },
            Err(reqwest_eventsource::Error::StreamEnded) => {
                tracing::debug!("SSE stream ended");
                break;
            },
            Err(e) => {
                anyhow::bail!("SSE stream error: {}", e);
            },
        }
    }

    let task = final_task.ok_or_else(|| anyhow!("Stream ended without final task"))?;

    let response = if accumulated_text.is_empty() {
        task.status
            .message
            .as_ref()
            .map(|msg| extract_text_from_parts(&msg.parts))
    } else {
        Some(accumulated_text)
    };

    let output = MessageOutput {
        agent: agent.to_string(),
        task,
        message_sent: message_text.to_string(),
        response,
    };

    Ok(CommandResult::card(output).with_title(format!("Message sent to {}", agent)))
}
