//! Buffering the caller's body before it goes upstream: the size cap and the
//! conversation-id derivation that reads the buffered bytes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use systemprompt_identifiers::GatewayConversationId;

use super::BUFFERED_BODY_LIMIT;
use super::error::{ForwardError, ForwardResult};
use crate::proxy::session::{self, SessionContext};

pub(super) async fn prepare_upstream_body(
    body: Incoming,
    session_context: &SessionContext,
) -> ForwardResult<(Bytes, Option<GatewayConversationId>)> {
    let buffered = collect_body(body).await?;
    let id = session::derive_gateway_conversation_id(&buffered)
        .map(|hash| session_context.context_for_prefix(hash));
    if let Some(ref c) = id {
        tracing::Span::current().record("gateway_conversation_id", tracing::field::display(c));
    }
    Ok((buffered, id))
}

async fn collect_body(body: Incoming) -> ForwardResult<Bytes> {
    match http_body_util::Limited::new(body, BUFFERED_BODY_LIMIT)
        .collect()
        .await
    {
        Ok(collected) => Ok(collected.to_bytes()),
        Err(e) if e.is::<http_body_util::LengthLimitError>() => Err(ForwardError::BodyTooLarge),
        Err(e) => Err(ForwardError::ReadBody(e)),
    }
}
