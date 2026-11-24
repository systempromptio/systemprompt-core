//! The structured error envelopes that carry more than a type and a message:
//! a quota refusal with its window detail, and a prompt-repair refusal with
//! the locations a client must change before retrying.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::body::Body;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use systemprompt_gateway::service::{GatewayError, QuotaExceeded};

use super::errors::{ERROR_TYPE_INVALID_REQUEST, build_error_response, policy_denial_message};

const PROMPT_REPAIR_ACTION: &str = "Remove secret-bearing content, correct system instructions, \
                                    or shorten the conversation before retrying";

pub(super) fn build_quota_exceeded(error: &GatewayError, quota: &QuotaExceeded) -> Response<Body> {
    let Some(detail) = quota.detail.as_ref() else {
        return build_error_response(error.status(), error.error_type(), &quota.message);
    };
    let body = serde_json::json!({
        "type": "error",
        "error": {
            "type": error.error_type(),
            "message": quota.message,
            "quota": detail,
        }
    });
    (error.status(), axum::Json(body)).into_response()
}

pub(super) fn build_prompt_repair(message: &str, locations: &[String]) -> Response<Body> {
    let body = serde_json::json!({
        "type": "error",
        "error": {
            "type": ERROR_TYPE_INVALID_REQUEST,
            "message": policy_denial_message(message),
            "recovery": {
                "code": "prompt_repair_required",
                "locations": locations,
                "retryable": false,
                "action": PROMPT_REPAIR_ACTION,
            }
        }
    });
    (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
}
