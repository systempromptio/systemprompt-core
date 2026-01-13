use anyhow::Result;
use clap::{Args, ValueEnum};
use systemprompt_core_logging::{AiTraceService, CliService};
use systemprompt_runtime::AppContext;

use super::ai_artifacts::print_artifacts;
use super::ai_display::{
    print_agent_response, print_ai_requests, print_conversation_history, print_execution_steps,
    print_system_prompt, print_task_info, print_user_input,
};
use super::ai_mcp::print_mcp_executions;
use crate::CliConfig;

#[derive(Clone, Copy, Default, ValueEnum)]
pub enum TraceOutput {
    #[default]
    Text,
    Json,
}

#[derive(Args)]
pub struct AiTraceOptions {
    #[arg(help = "Task ID (can be partial, will match prefix)")]
    pub task_id: String,

    #[arg(long, value_enum, default_value = "text")]
    pub output: TraceOutput,

    #[arg(
        long,
        help = "Include sections: history, artifact, tool-results (comma-separated)"
    )]
    pub include: Vec<TraceSection>,
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum TraceSection {
    History,
    Artifact,
    ToolResults,
}

impl AiTraceOptions {
    pub fn show_history(&self) -> bool {
        self.include.contains(&TraceSection::History)
    }

    pub fn show_artifact(&self) -> bool {
        self.include.contains(&TraceSection::Artifact)
    }

    pub fn show_tool_results(&self) -> bool {
        self.include.contains(&TraceSection::ToolResults)
    }
}

pub async fn execute(options: AiTraceOptions, config: &CliConfig) -> Result<()> {
    let _ = config; // Will be used when we convert to CommandResult
    let ctx = AppContext::new().await?;
    let pool = ctx.db_pool().pool_arc()?;

    let service = AiTraceService::new(pool);

    let task_id = match service.resolve_task_id(&options.task_id).await {
        Ok(id) => id,
        Err(_) => {
            CliService::warning(&format!("No task found matching: {}", options.task_id));
            CliService::info("Tip: Use 'systemprompt logs trace list' to see available traces");
            return Ok(());
        }
    };

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

    if options.show_history() {
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
        options.show_tool_results(),
    )
    .await;

    if options.show_artifact() {
        let artifacts = service.get_task_artifacts(&task_id, &context_id).await?;
        print_artifacts(&artifacts);
    }

    let response = service.get_agent_response(&task_id).await?;
    print_agent_response(response.as_ref());

    CliService::info("═".repeat(60).as_str());

    Ok(())
}
