use anyhow::{Context, Result};
use clap::Args;
use systemprompt_core_logging::{AiTraceService, CliService};
use systemprompt_runtime::AppContext;

use super::ai_artifacts::print_artifacts;
use super::ai_display::{
    print_agent_response, print_ai_requests, print_conversation_history, print_execution_steps,
    print_system_prompt, print_task_info, print_user_input,
};
use super::ai_mcp::print_mcp_executions;

#[derive(Args)]
pub struct AiTraceOptions {
    #[arg(help = "Task ID (can be partial, will match prefix)")]
    pub task_id: String,

    #[arg(long, help = "Show full conversation history")]
    pub history: bool,

    #[arg(long, help = "Show artifacts produced by the task")]
    pub artifact: bool,

    #[arg(long, help = "Output as JSON")]
    pub json: bool,

    #[arg(long, help = "Show full tool input/output")]
    pub tool_results: bool,
}

pub async fn execute(options: AiTraceOptions) -> Result<()> {
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let service = AiTraceService::new(pool);

    let task_id = service
        .resolve_task_id(&options.task_id)
        .await
        .context("Failed to find task")?;

    CliService::section(&format!("AI TRACE: {task_id}"));

    let task_info = service.get_task_info(&task_id).await?;
    let context_id = task_info.context_id.clone();

    print_task_info(&task_info);

    let user_input = service.get_user_input(&task_id).await?;
    print_user_input(user_input.as_ref());

    let steps = service.get_execution_steps(&task_id).await?;
    print_execution_steps(&steps);

    let ai_requests = service.get_ai_requests(&task_id).await?;
    let request_ids = print_ai_requests(&ai_requests);

    if let Some(first_id) = request_ids.first() {
        let prompt = service.get_system_prompt(first_id).await?;
        print_system_prompt(prompt.as_ref());
    }

    if options.history {
        let mut messages_by_request = Vec::new();
        for (idx, request_id) in request_ids.iter().enumerate() {
            let messages = service.get_conversation_messages(request_id).await?;
            messages_by_request.push((idx, messages));
        }
        print_conversation_history(&messages_by_request);
    }

    let mcp_executions = service.get_mcp_executions(&task_id, &context_id).await?;
    print_mcp_executions(
        &service,
        &mcp_executions,
        &task_id,
        &context_id,
        options.tool_results,
    )
    .await;

    if options.artifact {
        let artifacts = service.get_task_artifacts(&task_id, &context_id).await?;
        print_artifacts(&artifacts);
    }

    let response = service.get_agent_response(&task_id).await?;
    print_agent_response(response.as_ref());

    CliService::info("═".repeat(60).as_str());

    Ok(())
}
