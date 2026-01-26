use anyhow::{anyhow, Context, Result};
use clap::Args;
use futures_util::StreamExt;
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use systemprompt_agent::models::a2a::jsonrpc::{
    JsonRpcResponse, Request, RequestId, JSON_RPC_VERSION_2_0,
};
use systemprompt_agent::models::a2a::protocol::{
    MessageSendConfiguration, MessageSendParams, TaskStatusUpdateEvent,
};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_logging::CliService;
use systemprompt_models::a2a::{Message, Part, Task, TextPart};

use super::types::MessageOutput;
use crate::session::get_or_create_session;
use crate::interactive::resolve_required;
use crate::shared::CommandResult;
use crate::CliConfig;

#[derive(Debug, Args)]
pub struct MessageArgs {
    #[arg(help = "Agent name to send message to (required in non-interactive mode)")]
    pub agent: Option<String>,

    #[arg(short = 'm', long, help = "Message text to send")]
    pub message: Option<String>,

    #[arg(
        long,
        help = "Context ID for conversation continuity (overrides session)"
    )]
    pub context_id: Option<String>,

    #[arg(long, help = "Task ID to continue an existing task")]
    pub task_id: Option<String>,

    #[arg(long, help = "Gateway URL (overrides profile's api_external_url)")]
    pub url: Option<String>,

    #[arg(long, help = "Use streaming mode")]
    pub stream: bool,

    #[arg(long, help = "Wait for task completion (blocking mode)")]
    pub blocking: bool,

    #[arg(
        long,
        default_value = "30",
        help = "Timeout in seconds for blocking mode"
    )]
    pub timeout: u64,

    #[arg(long, help = "Output full task JSON instead of response text")]
    pub json: bool,
}

fn extract_text_from_parts(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text(text_part) => Some(text_part.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn execute(
    args: MessageArgs,
    config: &CliConfig,
) -> Result<CommandResult<MessageOutput>> {
    let session_ctx = get_or_create_session(config).await?;

    let agent = resolve_required(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    let message_text = resolve_required(args.message, "message", config, || {
        Err(anyhow!("Message text is required. Use -m or --message"))
    })?;

    let base_url = args
        .url
        .as_deref()
        .unwrap_or(&session_ctx.profile.server.api_external_url);
    let agent_url = format!("{}/api/v1/agents/{}", base_url.trim_end_matches('/'), agent);

    let context_id: ContextId = args
        .context_id
        .map_or_else(|| session_ctx.context_id().clone(), ContextId::new);
    let auth_token = session_ctx.session_token().as_str();

    let task_id: Option<TaskId> = args.task_id.map(TaskId::new);

    let message_id = MessageId::generate();
    let request_id = RequestId::String(MessageId::generate().to_string());

    let method = if args.stream {
        "message/stream"
    } else {
        "message/send"
    };

    let request = Request {
        jsonrpc: JSON_RPC_VERSION_2_0.to_string(),
        method: method.to_string(),
        params: MessageSendParams {
            message: Message {
                role: "user".to_string(),
                parts: vec![Part::Text(TextPart {
                    text: message_text.clone(),
                })],
                id: message_id,
                task_id,
                context_id: context_id.clone(),
                kind: "message".to_string(),
                metadata: None,
                extensions: None,
                reference_task_ids: None,
            },
            configuration: args.blocking.then_some(MessageSendConfiguration {
                blocking: Some(true),
                accepted_output_modes: None,
                history_length: None,
                push_notification_config: None,
            }),
            metadata: None,
        },
        id: request_id,
    };

    let use_json = args.json;

    let result = if args.stream {
        execute_streaming(&agent, &agent_url, auth_token, &request, &message_text).await?
    } else {
        execute_non_streaming(
            &agent,
            &agent_url,
            auth_token,
            &request,
            &message_text,
            args.timeout,
        )
        .await?
    };

    if use_json {
        return Ok(result);
    }

    let output = result.data;
    CliService::output(output.response.as_deref().unwrap_or("No response"));
    Ok(CommandResult::text(output).with_skip_render())
}

#[allow(clippy::print_stdout)]
async fn execute_streaming(
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
                                    print!("{}", text);
                                    accumulated_text.push_str(&text);
                                }
                            }

                            if event.is_final {
                                println!();
                                final_task = Some(Task {
                                    id: event.task_id.into(),
                                    context_id: event.context_id.into(),
                                    kind: "task".to_string(),
                                    status: event.status,
                                    history: None,
                                    artifacts: None,
                                    metadata: None,
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

#[allow(clippy::too_many_arguments)]
async fn execute_non_streaming(
    agent: &str,
    agent_url: &str,
    auth_token: &str,
    request: &Request<MessageSendParams>,
    message_text: &str,
    timeout: u64,
) -> Result<CommandResult<MessageOutput>> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .context("Failed to create HTTP client")?;

    let response = client
        .post(agent_url)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(request)
        .send()
        .await
        .with_context(|| format!("Failed to send message to agent at {}", agent_url))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_else(|_| String::new());
        anyhow::bail!("Agent request failed with status {}: {}", status, body);
    }

    let json_response: JsonRpcResponse<Task> = response
        .json()
        .await
        .context("Failed to parse agent response")?;

    if json_response.jsonrpc != JSON_RPC_VERSION_2_0 {
        anyhow::bail!(
            "Invalid JSON-RPC version: expected {}, got {}",
            JSON_RPC_VERSION_2_0,
            json_response.jsonrpc
        );
    }

    if let Some(error) = json_response.error {
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

    let task = json_response
        .result
        .ok_or_else(|| anyhow!("No result in agent response"))?;

    let response = task
        .status
        .message
        .as_ref()
        .map(|msg| extract_text_from_parts(&msg.parts));

    let output = MessageOutput {
        agent: agent.to_string(),
        task,
        message_sent: message_text.to_string(),
        response,
    };

    Ok(CommandResult::card(output).with_title(format!("Message sent to {}", agent)))
}
