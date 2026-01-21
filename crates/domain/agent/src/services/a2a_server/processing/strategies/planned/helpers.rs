use systemprompt_models::{AiMessage, AiRequest};

use super::super::ExecutionContext;

pub fn build_ai_request(context: &ExecutionContext, messages: Vec<AiMessage>) -> AiRequest {
    let tool_config = context.request_ctx.tool_model_config();

    let provider = tool_config
        .and_then(|c| c.provider.as_deref())
        .or(context.agent_runtime.provider.as_deref())
        .unwrap_or_else(|| context.ai_service.default_provider());
    let model = tool_config
        .and_then(|c| c.model.as_deref())
        .or(context.agent_runtime.model.as_deref())
        .unwrap_or_else(|| context.ai_service.default_model());
    let max_output_tokens = tool_config
        .and_then(|c| c.max_output_tokens)
        .or(context.agent_runtime.max_output_tokens)
        .unwrap_or_else(|| context.ai_service.default_max_output_tokens());

    if tool_config.is_some() {
        tracing::debug!(
            provider,
            model,
            max_output_tokens,
            "Using tool_model_config in planned strategy"
        );
    }

    AiRequest::builder(
        messages,
        provider,
        model,
        max_output_tokens,
        context.request_ctx.clone(),
    )
    .build()
}
