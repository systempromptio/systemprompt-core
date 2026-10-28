//! Who an upload call acts as: the session's user, or the subject of a
//! signed upload token the gate verified, and the trace it is logged under.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::Extensions;
use systemprompt_identifiers::{TraceId, UserId};
use systemprompt_models::RequestContext;

use crate::error::ApiHttpError;

#[derive(Debug, Clone)]
pub(super) struct Uploader {
    pub(super) user: UserId,
    pub(super) trace: TraceId,
}

/// Set by the token gate on a request it authorized from a signed URL.
#[derive(Debug, Clone)]
pub(super) struct TokenUploader(pub(super) Uploader);

pub(super) fn uploader(extensions: &Extensions) -> Result<Uploader, ApiHttpError> {
    if let Some(TokenUploader(uploader)) = extensions.get::<TokenUploader>() {
        return Ok(uploader.clone());
    }
    let ctx = extensions
        .get::<RequestContext>()
        .ok_or_else(|| ApiHttpError::unauthorized("upload routes need an authenticated session"))?;
    Ok(Uploader {
        user: ctx.user_id().clone(),
        trace: ctx.trace_id().clone(),
    })
}
