//! The `gateway-http` member's surface as the gateway router mounts it: with
//! the member admitted its inference, token-count, telemetry and
//! personal-credential routes answer an unauthenticated caller 401; with it
//! disabled they are not mounted (404) while the bridge routes the API still
//! serves keep answering, the Drive routes included: they depend on the
//! `documents` member alone.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_identifiers::ExtensionId;
use systemprompt_test_fixtures::{
    Probe, ensure_test_bootstrap, fixture_admitted, fixture_admitted_of, fixture_admitted_without,
    fixture_app_context_with_extensions, fixture_config, fixture_core_members,
};
use systemprompt_traits::{
    DriveSyncGrant, DriveSyncPrincipal, DriveTokenError, DriveTokens, DynDriveTokens,
};
use tower::ServiceExt;

const INFERENCE: [&str; 4] = [
    "/messages",
    "/responses",
    "/chat/completions",
    "/messages/count_tokens",
];

const CREDENTIALS: &str = "/me/provider-credentials";
const OTEL: &str = "/otel/v1/traces";

async fn router(admitted: std::sync::Arc<systemprompt_extension::Admitted>) -> Router {
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context_with_extensions(fixture_config(&boot.database_url), admitted)
        .await
        .expect("context builds");
    gateway_router(&ctx, ctx.admitted(), ctx.kernel()).expect("gateway router builds")
}

async fn call(app: &Router, method: &str, uri: &str, bearer: Option<&str>) -> (u16, String) {
    let mut req = Request::builder()
        .method(Method::from_bytes(method.as_bytes()).expect("method"))
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let req = req.body(Body::from("{}")).expect("request");
    let resp = app.clone().oneshot(req).await.expect("response");
    let status = resp.status().as_u16();
    let body = axum::body::to_bytes(resp.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, String::from_utf8_lossy(&body).into_owned())
}

async fn status(app: &Router, method: &str, uri: &str, bearer: Option<&str>) -> u16 {
    call(app, method, uri, bearer).await.0
}

#[tokio::test]
async fn the_admitted_member_serves_its_inference_handlers() {
    let app = router(fixture_admitted()).await;
    for uri in INFERENCE {
        let (status, body) = call(&app, "POST", uri, None).await;
        assert_eq!(status, 404, "{uri}");
        assert!(
            body.contains("Gateway not enabled"),
            "{uri} is mounted: {body}"
        );
    }
}

#[tokio::test]
async fn the_credential_and_telemetry_routes_refuse_a_caller_without_a_credential() {
    let app = router(fixture_admitted()).await;
    assert_eq!(status(&app, "GET", CREDENTIALS, None).await, 401);
    assert_eq!(
        status(&app, "GET", CREDENTIALS, Some("not-a-token")).await,
        401
    );
    assert_eq!(status(&app, "POST", OTEL, None).await, 401);
}

#[tokio::test]
async fn a_disabled_gateway_http_member_mounts_none_of_its_routes() {
    let app = router(fixture_admitted_without(&[ExtensionId::new(
        "gateway-http",
    )]))
    .await;
    for uri in INFERENCE {
        let (status, body) = call(&app, "POST", uri, None).await;
        assert_eq!(status, 404, "{uri}");
        assert!(
            !body.contains("Gateway not enabled"),
            "{uri} is not mounted: {body}"
        );
    }
    assert_eq!(status(&app, "GET", CREDENTIALS, None).await, 404);
    assert_eq!(status(&app, "POST", OTEL, None).await, 404);
    assert_eq!(
        status(&app, "GET", "/bridge/pubkey", None).await,
        200,
        "the bridge routes the API serves stay mounted"
    );
    assert_eq!(
        status(&app, "GET", DRIVE_MANIFEST, None).await,
        401,
        "the Drive routes stay mounted and refuse a caller without a Drive token"
    );
}

const DRIVE_MANIFEST: &str = "/bridge/drive/manifest";

#[tokio::test]
async fn the_drive_routes_depend_on_the_documents_member() {
    let app = router(fixture_admitted()).await;
    assert_eq!(status(&app, "GET", DRIVE_MANIFEST, None).await, 401);
    let app = router(fixture_admitted_without(&[ExtensionId::new("documents")])).await;
    assert_eq!(status(&app, "GET", DRIVE_MANIFEST, None).await, 404);
}

#[tokio::test]
async fn a_disabled_ai_member_withholds_only_the_personal_credential_routes() {
    let app = router(fixture_admitted_without(&[ExtensionId::new("ai")])).await;
    assert_eq!(status(&app, "GET", CREDENTIALS, None).await, 404);
    assert_eq!(status(&app, "POST", OTEL, None).await, 401);
}

#[derive(Debug)]
struct ForeignDriveTokens;

#[async_trait::async_trait]
impl DriveTokens for ForeignDriveTokens {
    async fn trade(&self, _device_token: &str) -> Result<DriveSyncGrant, DriveTokenError> {
        Err(DriveTokenError::NotDeviceToken)
    }

    async fn verify(&self, _drive_token: &str) -> Result<DriveSyncPrincipal, DriveTokenError> {
        Err(DriveTokenError::NotDeviceToken)
    }
}

#[tokio::test]
async fn a_drive_token_view_from_a_member_other_than_documents_is_refused() {
    let members = fixture_core_members()
        .into_iter()
        .filter(|member| member.metadata().id != "documents")
        .chain(std::iter::once(Box::new(Probe::new("acme-drive", |c| {
            c.provide(
                    |_r: &systemprompt_extension::prelude::Resolved<'_>| -> Result<
                        DynDriveTokens,
                        systemprompt_extension::prelude::ProvideError,
                    > { Ok(std::sync::Arc::new(ForeignDriveTokens)) },
                );
        }))
            as Box<dyn systemprompt_extension::Extension>))
        .collect();
    let boot = ensure_test_bootstrap();
    let ctx = fixture_app_context_with_extensions(
        fixture_config(&boot.database_url),
        fixture_admitted_of(members),
    )
    .await
    .expect("context builds");
    let app = gateway_router(&ctx, ctx.admitted(), ctx.kernel()).expect("gateway router builds");
    assert_eq!(
        status(&app, "GET", DRIVE_MANIFEST, None).await,
        404,
        "a Drive token view the documents member did not provide never serves the Drive routes"
    );
}
