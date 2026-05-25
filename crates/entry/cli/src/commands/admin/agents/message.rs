use anyhow::{Result, anyhow};
use clap::Args;
use systemprompt_agent::models::a2a::jsonrpc::{JSON_RPC_VERSION_2_0, Request, RequestId};
use systemprompt_agent::models::a2a::protocol::{MessageSendConfiguration, MessageSendParams};
use systemprompt_identifiers::{ContextId, MessageId, TaskId};
use systemprompt_logging::CliService;
use systemprompt_models::a2a::{Message, MessageRole, Part, TextPart, methods};

use super::client::ensure_agent_exists;
use super::message_request::{NonStreamingRequest, execute_non_streaming};
use super::message_streaming::execute_streaming;
use super::types::MessageOutput;
use crate::CliConfig;
use crate::interactive::resolve_required;
use crate::session::get_or_create_session;
use crate::shared::CommandResult;

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  systemprompt admin agents message developer_agent \\\n      -m \
                  'Use --safe-mode when running migrations.' --blocking\n\n  Always single-quote \
                  the -m value so flag-like tokens (--foo) inside the\n  message are not \
                  interpreted by the shell or by clap."
)]
pub struct MessageArgs {
    #[arg(help = "Agent name to send message to (required in non-interactive mode)")]
    pub agent: Option<String>,

    #[arg(short = 'm', long, help = "Message text to send")]
    pub message: Option<String>,

    #[arg(
        long = "context-id",
        help = "Context ID for conversation continuity (overrides session)"
    )]
    pub context: Option<String>,

    #[arg(long = "task-id", help = "Task ID to continue an existing task")]
    pub task: Option<String>,

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

pub(super) fn extract_text_from_parts(parts: &[Part]) -> String {
    parts
        .iter()
        .filter_map(|part| match part {
            Part::Text(text_part) => Some(text_part.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) async fn execute(
    args: MessageArgs,
    config: &CliConfig,
) -> Result<CommandResult<MessageOutput>> {
    let session_ctx = get_or_create_session(config).await?;

    let agent = resolve_required(args.agent, "agent", config, || {
        Err(anyhow!("Agent name is required"))
    })?;

    ensure_agent_exists(&agent)?;

    let message_text = resolve_required(args.message, "message", config, || {
        Err(anyhow!("Message text is required. Use -m or --message"))
    })?;

    let base_url = args
        .url
        .as_deref()
        .unwrap_or(&session_ctx.profile.server.api_external_url);
    let agent_url = format!("{}/api/v1/agents/{}", base_url.trim_end_matches('/'), agent);

    let context_id: ContextId = args
        .context
        .and_then(|s| ContextId::try_new(s).ok())
        .unwrap_or_else(|| session_ctx.context_id().clone());
    let auth_token = session_ctx.session_token().as_str();

    let task_id: Option<TaskId> = args.task.map(TaskId::new);

    let message_id = MessageId::generate();
    let request_id = RequestId::String(MessageId::generate().to_string());

    let method = if args.stream {
        methods::SEND_STREAMING_MESSAGE
    } else {
        methods::SEND_MESSAGE
    };

    let request = Request {
        jsonrpc: JSON_RPC_VERSION_2_0.to_string(),
        method: method.to_string(),
        params: MessageSendParams {
            message: Message {
                role: MessageRole::User,
                parts: vec![Part::Text(TextPart {
                    text: message_text.clone(),
                })],
                message_id,
                task_id,
                context_id: context_id.clone(),
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
        execute_non_streaming(NonStreamingRequest {
            agent: &agent,
            agent_url: &agent_url,
            auth_token,
            request: &request,
            message_text: &message_text,
            timeout: args.timeout,
        })
        .await?
    };

    if use_json {
        return Ok(result);
    }

    let output = result.data;
    CliService::output(output.response.as_deref().unwrap_or("No response"));
    Ok(CommandResult::text(output).with_skip_render())
}
