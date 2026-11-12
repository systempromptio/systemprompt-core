//! A success status whose body the gateway cannot serve: one that carries no
//! turn, or one the wire codec cannot parse. Both are answered as a 502 that
//! keeps an excerpt of what the upstream sent.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{OutboundError, UpstreamError};

const DEFECTIVE_BODY_STATUS: u16 = 502;

pub(crate) fn reject_defective_body(
    provider: &str,
    wire: &str,
    defect: &systemprompt_wire::defect::BodyDefect,
    body: &bytes::Bytes,
) -> OutboundError {
    let excerpt: String = String::from_utf8_lossy(body).chars().take(512).collect();
    tracing::warn!(
        provider = %provider,
        wire = %wire,
        defect = %defect,
        body = %excerpt,
        "upstream returned a success status with a body carrying no turn"
    );
    OutboundError::Upstream(UpstreamError::Status {
        provider: provider.to_owned(),
        status: DEFECTIVE_BODY_STATUS,
        message: format!("{defect}: {excerpt}"),
        body: Box::new(body.clone()),
        retry_after: None,
        request_id: None,
    })
}

pub(crate) fn reject_unparsable_body(
    provider: &str,
    wire: &str,
    error: &systemprompt_wire::error::WireParseError,
    body: &bytes::Bytes,
) -> OutboundError {
    let excerpt: String = String::from_utf8_lossy(body).chars().take(512).collect();
    tracing::error!(
        provider = %provider,
        wire = %wire,
        error = %error,
        body = %excerpt,
        "upstream returned a success status with a body that does not parse"
    );
    OutboundError::Upstream(UpstreamError::Status {
        provider: provider.to_owned(),
        status: DEFECTIVE_BODY_STATUS,
        message: format!("{error}: {excerpt}"),
        body: Box::new(body.clone()),
        retry_after: None,
        request_id: None,
    })
}
