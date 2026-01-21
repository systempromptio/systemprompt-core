use systemprompt_logging::{RequestSpan, RequestSpanBuilder};
use systemprompt_models::RequestContext;

pub fn create_request_span(ctx: &RequestContext) -> RequestSpan {
    let mut builder = RequestSpanBuilder::new(
        &ctx.auth.user_id,
        &ctx.request.session_id,
        &ctx.execution.trace_id,
    );

    if !ctx.execution.context_id.as_str().is_empty() {
        builder = builder.with_context_id(&ctx.execution.context_id);
    }

    if let Some(ref task_id) = ctx.execution.task_id {
        builder = builder.with_task_id(task_id);
    }

    if let Some(ref client_id) = ctx.request.client_id {
        builder = builder.with_client_id(client_id);
    }

    builder.build()
}
