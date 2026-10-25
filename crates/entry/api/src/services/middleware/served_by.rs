//! `x-served-by` replica-identification header middleware.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_identifiers::InstanceId;

pub async fn inject_served_by(
    State(instance_id): State<InstanceId>,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;

    if let Ok(header_value) = instance_id.as_str().parse() {
        response.headers_mut().insert("x-served-by", header_value);
    }

    response
}
