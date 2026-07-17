//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::{AiMessage, AiRequest};

use super::super::ExecutionContext;
use crate::services::a2a_server::processing::ai_executor::resolve_provider_config;

pub(super) fn build_ai_request(context: &ExecutionContext, messages: Vec<AiMessage>) -> AiRequest {
    let (provider, model, max_output_tokens) = resolve_provider_config(
        &context.request_ctx,
        &context.agent_runtime,
        context.ai_service.as_ref(),
    );

    AiRequest::builder(
        messages,
        &provider,
        &model,
        max_output_tokens,
        context.request_ctx.clone(),
    )
    .build()
}
