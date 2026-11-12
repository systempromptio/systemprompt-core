//! Finalising an upstream answer that is returned whole: the upstream's
//! headers and bytes go to the client, the parsed or decoded copy goes to the
//! audit row. A buffered answer the response safety policy blocks is the one
//! case the client does not receive it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::body::Body;
use axum::response::Response;
use http::{HeaderValue, StatusCode};

use super::finalize_buffered;
use crate::audit::GatewayAudit;
use crate::protocol::inbound::InboundAdapter;
use crate::protocol::outbound::{Relayed, RelayedBody};
use crate::stream_tap;

pub(super) async fn finalize_relayed(
    relayed: Relayed,
    inbound: Arc<dyn InboundAdapter>,
    audit: Arc<GatewayAudit>,
    tap_ctx: stream_tap::TapFinalizeCtx,
) -> Response<Body> {
    let Relayed { headers, body } = relayed;
    let mut response = match body {
        RelayedBody::Buffered { bytes, canonical } => {
            let served =
                finalize_buffered(*canonical, bytes, "application/json", &audit, tap_ctx).await;
            if served.status() != StatusCode::OK {
                return served;
            }
            served
        },
        RelayedBody::Streaming { bytes, events } => {
            let mut streamed = Response::new(stream_tap::tap_relayed(
                bytes, events, inbound, audit, tap_ctx,
            ));
            // Why: a reverse proxy in front of this hop buffers a response
            // unless told not to, which turns a stream into one late body.
            streamed
                .headers_mut()
                .insert("x-accel-buffering", HeaderValue::from_static("no"));
            streamed
        },
    };
    response.headers_mut().extend(headers);
    response
}
