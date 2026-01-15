use super::ApiError;
use crate::execution::context::RequestContext;

pub trait ApiErrorExt {
    fn with_request_context(self, ctx: &RequestContext) -> Self;
}

impl ApiErrorExt for ApiError {
    fn with_request_context(self, ctx: &RequestContext) -> Self {
        self.with_trace_id(ctx.trace_id().to_string())
    }
}
