use std::sync::Arc;
use systemprompt_test_fixtures::AppContextSeeds as _;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{HeaderMap, Method, Request, StatusCode};
use systemprompt_api::services::middleware::{
    ContextExtractor, RouterExt, UserOnlyContextMiddleware,
};
use systemprompt_extension::AuthzPolicy;
use systemprompt_extension::router::RouteMethod;
use systemprompt_identifiers::headers::DEVICE_FINGERPRINT;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_models::modules::ApiPaths;
use systemprompt_notifications_entry::http::{admin_routes, user_routes};
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, seed_user_row, test_app_context, unique_user_id,
};
use systemprompt_users::{
    DeviceCertService, EnrollDeviceCertServiceParams, UserDeviceCert, UserRepository,
};
use tower::ServiceExt;
use uuid::Uuid;

use crate::support::{admin_router, user_router};

#[derive(Clone)]
struct Caller(Option<RequestContext>);

#[async_trait::async_trait]
impl ContextExtractor for Caller {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.0
            .clone()
            .ok_or(ContextExtractionError::MissingAuthHeader)
    }
}

fn caller(user: &UserId) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("device-routes").expect("agent name"),
        Actor::user(user.clone()),
    )
    .with_user_type(UserType::User)
}

async fn context() -> Arc<AppContext> {
    let boot = ensure_test_bootstrap();
    test_app_context(&boot.database_url).await
}

fn app(ctx: &AppContext, user: Option<&UserId>) -> Router {
    user_router(ctx).with_auth(
        UserOnlyContextMiddleware::new(Caller(user.map(caller))),
        AuthzPolicy::user(),
    )
}

async fn call(
    app: Router,
    method: Method,
    uri: &str,
    fingerprint: Option<&str>,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(fingerprint) = fingerprint {
        builder = builder.header(DEVICE_FINGERPRINT, fingerprint);
    }
    let request = match body {
        Some(json) => builder
            .header("content-type", "application/json")
            .body(Body::from(json.to_string())),
        None => builder.body(Body::empty()),
    }
    .expect("request");
    let response = app.oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.expect("body");
    let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, json)
}

fn certs(ctx: &AppContext) -> DeviceCertService {
    DeviceCertService::new(Arc::new(UserRepository::new(ctx.db_pool())))
}

fn unique_fingerprint() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

async fn user_with_device(ctx: &AppContext, prefix: &str) -> (UserId, String, UserDeviceCert) {
    let user = unique_user_id(prefix);
    seed_user_row(
        ctx.db_pool(),
        &user,
        &format!("{}@device-routes.invalid", user.as_str()),
    )
    .await
    .expect("seed user");
    let fingerprint = unique_fingerprint();
    let device = certs(ctx)
        .enroll(EnrollDeviceCertServiceParams {
            user_id: &user,
            fingerprint: &fingerprint,
            label: "phone",
        })
        .await
        .expect("enrol device");
    (user, fingerprint, device)
}

fn registration(token: &str) -> serde_json::Value {
    serde_json::json!({
        "provider": "apns",
        "token": token,
        "client": {
            "platform": "ios",
            "os_version": "18.2",
            "app_version": "1.4.0",
            "app_id": "io.example.app",
        },
    })
}

fn token() -> String {
    Uuid::new_v4().simple().to_string()
}

#[test]
fn the_routes_sit_at_the_contract_paths() {
    let user_base = ApiPaths::NOTIFICATIONS_PUSH_TOKEN
        .strip_suffix("/push-token")
        .expect("the push token path ends the notifications base");
    let admin_base = ApiPaths::ADMIN_NOTIFICATIONS_DEVICES
        .strip_suffix("/devices")
        .expect("the device list path ends the admin notifications base");
    assert!(ApiPaths::NOTIFICATIONS_PUSH_TOKEN.starts_with(user_base));
    assert!(ApiPaths::ADMIN_NOTIFICATIONS_DEVICES.starts_with(admin_base));

    let methods = |set: &systemprompt_extension::router::RouteSet, path: &str| {
        set.defs()
            .iter()
            .filter(|def| def.path.as_str() == path)
            .map(|def| match &def.method {
                RouteMethod::One(method) => method.to_string(),
                RouteMethod::Any => "ANY".to_owned(),
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        methods(&user_routes(), "/push-token"),
        vec!["PUT".to_owned(), "DELETE".to_owned()]
    );
    assert_eq!(methods(&admin_routes(), "/devices"), vec!["GET".to_owned()]);
}

#[tokio::test]
async fn the_push_token_routes_require_a_signed_in_user() {
    let ctx = context().await;
    for method in [Method::PUT, Method::DELETE] {
        let (status, _) = call(
            app(&ctx, None),
            method,
            "/push-token",
            Some(&unique_fingerprint()),
            Some(registration(&token())),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn a_request_without_a_device_fingerprint_is_refused() {
    let ctx = context().await;
    let (user, _, _) = user_with_device(&ctx, "dr1").await;

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        None,
        Some(registration(&token())),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error_key"], "device_fingerprint_required");

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        Some("not-a-fingerprint"),
        Some(registration(&token())),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error_key"], "invalid_device_fingerprint");
}

#[tokio::test]
async fn a_device_registers_refreshes_and_revokes_its_own_token() {
    let ctx = context().await;
    let (user, fingerprint, device) = user_with_device(&ctx, "dr2").await;

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        Some(&fingerprint),
        Some(registration(&token())),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], device.id.as_str());
    assert_eq!(body["provider"], "apns");
    assert!(body.get("token").is_none(), "the token is not echoed");

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        Some(&fingerprint),
        Some(registration(&token())),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], device.id.as_str());

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::DELETE,
        "/push-token",
        Some(&fingerprint),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["device_id"], device.id.as_str());
    assert_eq!(body["revoked"], true);

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::DELETE,
        "/push-token",
        Some(&fingerprint),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["revoked"], false);
}

#[tokio::test]
async fn a_caller_cannot_write_another_users_device() {
    let ctx = context().await;
    let (_, fingerprint, _) = user_with_device(&ctx, "dr3").await;
    let (intruder, _, _) = user_with_device(&ctx, "dr3").await;

    for (method, body) in [
        (Method::PUT, Some(registration(&token()))),
        (Method::DELETE, None),
    ] {
        let (status, body) = call(
            app(&ctx, Some(&intruder)),
            method,
            "/push-token",
            Some(&fingerprint),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
        assert_eq!(body["error_key"], "device_not_enrolled");
    }
}

#[tokio::test]
async fn a_revoked_device_cannot_register() {
    let ctx = context().await;
    let (user, fingerprint, device) = user_with_device(&ctx, "dr4").await;
    assert!(
        certs(&ctx)
            .revoke(&device.id, &user)
            .await
            .expect("revoke device")
    );

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        Some(&fingerprint),
        Some(registration(&token())),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error_key"], "device_not_enrolled");
}

#[tokio::test]
async fn client_info_the_provider_cannot_serve_is_a_bad_request() {
    let ctx = context().await;
    let (user, fingerprint, _) = user_with_device(&ctx, "dr5").await;
    let mut body = registration(&token());
    body["client"]["platform"] = serde_json::json!("android");

    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        Some(&fingerprint),
        Some(body),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error_key"], "invalid_client_info");
}

#[tokio::test]
async fn the_admin_list_shows_platform_and_app_version_without_the_token() {
    let ctx = context().await;
    let (user, fingerprint, device) = user_with_device(&ctx, "dr6").await;
    let secret = token();
    let (status, body) = call(
        app(&ctx, Some(&user)),
        Method::PUT,
        "/push-token",
        Some(&fingerprint),
        Some(registration(&secret)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, body) = call(
        admin_router(&ctx),
        Method::GET,
        &format!("/devices?user_id={}", user.as_str()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let devices = body["devices"].as_array().expect("devices");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["device_id"], device.id.as_str());
    assert_eq!(devices[0]["user_id"], user.as_str());
    assert_eq!(devices[0]["platform"], "ios");
    assert_eq!(devices[0]["os_version"], "18.2");
    assert_eq!(devices[0]["app_version"], "1.4.0");
    assert_eq!(devices[0]["app_id"], "io.example.app");
    assert_eq!(devices[0]["push_provider"], "apns");
    assert!(
        !body.to_string().contains(&secret),
        "the token is never listed"
    );
}
