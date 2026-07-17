//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_models::Config;

pub async fn inject_served_by(request: Request, next: Next) -> Response {
    let instance_id = Config::get().ok().map(|cfg| cfg.instance_id.clone());

    let mut response = next.run(request).await;

    if let Some(id) = instance_id
        && let Ok(header_value) = id.parse()
    {
        response.headers_mut().insert("x-served-by", header_value);
    }

    response
}
