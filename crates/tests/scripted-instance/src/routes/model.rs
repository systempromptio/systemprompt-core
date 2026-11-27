//! The model routes: one scripted turn per request, rendered in the
//! protocol of the route called. The recording layer admits the request
//! and takes its turn; a request without a credential gets none.

use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::Value;
use systemprompt_wire::WireProtocol;

use crate::encode::{self, Call};
use crate::script::TurnFault;
use crate::state::{Admitted, Decision};

const EVENT_STREAM: &str = "text/event-stream";
const JSON: &str = "application/json";
const UNNAMED_MODEL: &str = "scripted-model";

// Why: a client that backs off on a scripted 429 or 5xx would otherwise hold
// the scenario for its real backoff; the wire names no constant for this
// provider header.
const RETRY_AFTER_MS: &str = "retry-after-ms";
const RETRY_NOW: &str = "1";

pub(crate) async fn messages(Extension(admitted): Extension<Admitted>) -> Response {
    answer(&admitted, WireProtocol::Anthropic)
}

pub(crate) async fn chat(Extension(admitted): Extension<Admitted>) -> Response {
    answer(&admitted, WireProtocol::OpenAiChat)
}

fn answer(admitted: &Admitted, protocol: WireProtocol) -> Response {
    let error = |status: StatusCode, message: &str| {
        let body = encode::error_body(protocol, status.as_u16(), message);
        (status, Json(body)).into_response()
    };
    let turn = match &admitted.decision {
        Some(Decision::Turn(turn)) => turn,
        Some(Decision::Refused(reason)) => {
            return error(
                StatusCode::BAD_REQUEST,
                &format!("scripted instance: {reason}"),
            );
        },
        None => {
            return error(
                StatusCode::UNAUTHORIZED,
                "scripted instance: a session token or the personal access token is required",
            );
        },
    };
    if let Some(TurnFault::Status(status)) = turn.fault {
        let mut refused = error(
            StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            "scripted instance: the turn answers with this status",
        );
        refused
            .headers_mut()
            .insert(RETRY_AFTER_MS, HeaderValue::from_static(RETRY_NOW));
        return refused;
    }
    let request = admitted.json();
    let call = Call {
        protocol,
        model: request
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(UNNAMED_MODEL),
        sequence: admitted.sequence,
    };
    if request.get("stream").and_then(Value::as_bool) == Some(true) {
        let steps = encode::stream_steps(turn, call);
        (
            [(CONTENT_TYPE, EVENT_STREAM), (CACHE_CONTROL, "no-cache")],
            encode::body(steps),
        )
            .into_response()
    } else {
        let steps = encode::buffered_steps(turn, call);
        ([(CONTENT_TYPE, JSON)], encode::body(steps)).into_response()
    }
}
