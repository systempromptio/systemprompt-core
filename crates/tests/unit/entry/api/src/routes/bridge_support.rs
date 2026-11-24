use axum::body::Body;
use axum::http::{HeaderMap, Request};
use axum::response::Response;
use systemprompt_extension::reserved::CoreId;
use systemprompt_gateway::{GatewayRouteState, GatewaySurface, GatewaySurfaceRoutes};
use systemprompt_gateway_entry::http::GatewayHttpSurface;
use systemprompt_identifiers::ExtensionId;
use systemprompt_models::ApiPaths;
use systemprompt_models::api::ApiError;
use systemprompt_runtime::AppContext;
use tower::ServiceExt;

pub fn surface_routes(ctx: &AppContext) -> GatewaySurfaceRoutes {
    let owner = ExtensionId::new(CoreId::GatewayHttp.id());
    let standing = ctx
        .admitted()
        .built()
        .standing_of(&owner)
        .expect("gateway-http is admitted");
    GatewayHttpSurface
        .build(GatewayRouteState {
            resolved: ctx.kernel().route_scoped(&owner, standing, &[]),
            host: systemprompt_api::routes::gateway::host_handles(ctx).expect("host handles"),
        })
        .expect("the gateway surface builds")
}

pub async fn manifest(ctx: &AppContext, headers: HeaderMap) -> Result<Response, ApiError> {
    let app = systemprompt_api::routes::gateway::gateway_router(ctx, ctx.admitted(), ctx.kernel())
        .expect("gateway router");
    let mut request = Request::builder()
        .uri(ApiPaths::gateway_relative(ApiPaths::BRIDGE_MANIFEST))
        .body(Body::empty())
        .expect("request");
    *request.headers_mut() = headers;
    let response = app
        .oneshot(request)
        .await
        .expect("the manifest route answers");
    if response.status().is_success() || response.status().is_redirection() {
        return Ok(response);
    }
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("error body");
    Err(serde_json::from_slice(&bytes).expect("an API error body"))
}
