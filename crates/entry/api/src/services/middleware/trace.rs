use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;
use systemprompt_models::RequestContext;

pub async fn inject_trace_header(request: Request, next: Next) -> Response {
    let trace_id = request
        .extensions()
        .get::<RequestContext>()
        .map(|ctx| ctx.trace_id().as_str().to_string());

    let mut response = next.run(request).await;

    if let Some(id) = trace_id {
        if let Ok(header_value) = id.parse() {
            response.headers_mut().insert("x-trace-id", header_value);
        }
    }

    response
}
