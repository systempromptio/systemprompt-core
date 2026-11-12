//! Contains a panic raised while serving a request to that request.
//!
//! The client receives the standard fixed-message 500 envelope; the panic
//! message is logged as a structured field and never reaches the body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::any::Any;

use axum::response::{IntoResponse, Response};
use tower_http::catch_panic::CatchPanicLayer;

use crate::error::ApiHttpError;

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("request handler panicked")]
pub struct HandlerPanicked;

type PanicHandler = fn(Box<dyn Any + Send + 'static>) -> Response;

pub fn panic_fence() -> CatchPanicLayer<PanicHandler> {
    let handler: PanicHandler = |payload| panic_response(payload.as_ref());
    CatchPanicLayer::custom(handler)
}

fn panic_response(payload: &(dyn Any + Send)) -> Response {
    let message = payload
        .downcast_ref::<&'static str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned());
    tracing::error!(panic = %message, "Request handler panicked");
    ApiHttpError::internal("request handler panicked", HandlerPanicked).into_response()
}
