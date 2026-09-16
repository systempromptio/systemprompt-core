//! Bound external identifiers and JSON collection depth before typed
//! extraction.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.
use super::problem;
use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::Response;
const BODY_LIMIT: usize = 1024 * 1024;
pub(super) async fn validate(request: Request) -> Result<Request, Response> {
    if request.uri().path().split('/').any(|part| part.len() > 512)
        || request.uri().query().is_some_and(|query| {
            query.len() > 8192
                || url::form_urlencoded::parse(query.as_bytes())
                    .any(|(key, value)| key.len() > 128 || value.len() > 512)
        })
    {
        return Err(problem(
            StatusCode::BAD_REQUEST,
            "Path identifiers and cursor parameters exceed their bounds",
        ));
    }
    if !request
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(';').next().is_some_and(|mime| {
                let mime = mime.trim().to_ascii_lowercase();
                mime == "application/json"
                    || (mime.starts_with("application/") && mime.ends_with("+json"))
            })
        })
    {
        return Ok(request);
    }
    let (parts, body) = request.into_parts();
    let bytes = to_bytes(body, BODY_LIMIT)
        .await
        .map_err(|_error| problem(StatusCode::PAYLOAD_TOO_LARGE, "JSON request exceeds 1 MiB"))?;
    // JSON: inspect the bounded transport envelope before its typed extractor;
    // values remain immutable and are not used as application contracts.
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
        check(&value, None, 0).map_err(|message| problem(StatusCode::BAD_REQUEST, message))?;
    }
    Ok(Request::from_parts(parts, Body::from(bytes)))
}
// JSON: recursively enforce transport limits before typed request extraction.
fn check(value: &serde_json::Value, key: Option<&str>, depth: usize) -> Result<(), &'static str> {
    if depth > 32 {
        return Err("JSON nesting exceeds 32 levels");
    }
    match value {
        serde_json::Value::String(value) => {
            let limit = if key
                .is_some_and(|key| key == "id" || key.ends_with("_id") || key == "after")
            {
                512
            } else if key.is_some_and(|key| key.contains("idempotency") || key == "operation_key") {
                200
            } else {
                65536
            };
            if value.len() > limit
                || value.contains('\0')
                || (value.is_empty()
                    && key.is_some_and(|key| {
                        key == "id"
                            || key.ends_with("_id")
                            || key == "after"
                            || key.contains("idempotency")
                            || key == "operation_key"
                    }))
            {
                return Err("String or identifier exceeds its contract bounds");
            }
        },
        serde_json::Value::Array(values) => {
            if values.len() > 8192 {
                return Err("JSON collection exceeds 8192 items");
            }
            for value in values {
                check(value, key, depth + 1)?;
            }
        },
        serde_json::Value::Object(values) => {
            if values.len() > 8192 {
                return Err("JSON object exceeds 8192 members");
            }
            for (key, value) in values {
                check(value, Some(key), depth + 1)?;
            }
        },
        _ => {},
    }
    Ok(())
}
