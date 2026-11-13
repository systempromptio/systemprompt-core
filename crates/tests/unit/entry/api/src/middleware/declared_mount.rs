//! `protect` — the one function that applies a route declaration. Covers every
//! policy × context pairing the server mounts, the refusals for a policy that
//! admits nobody and for a bearer window or protection layer with no
//! primitive, and that the rate limit, body limit and frame options a
//! declaration names are applied. The limiter is built over the fixture
//! database.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, ContextExtractor, DEFAULT_BODY_LIMIT_BYTES,
    McpContextMiddleware, MountError, MountLayers, ProtectionGuards, PublicContextMiddleware,
    RateLimitState, UserOnlyContextMiddleware, admit_extension, protect,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, FrameOptionsOverride, ProtectionLayer, RateBudget,
    RateLimit, RouteProtection,
};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_manifest::profile::{FrameOptions, RateLimitsConfig};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_test_fixtures::{fixture_config, test_db_pool};
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;

const BASE: &str = "/api/v1/declared";

#[derive(Clone)]
struct StubExtractor {
    context: Option<RequestContext>,
}

#[async_trait::async_trait]
impl ContextExtractor for StubExtractor {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        self.context
            .clone()
            .ok_or(ContextExtractionError::MissingAuthHeader)
    }
}

fn context(kind: UserType) -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("declared").expect("valid AgentName"),
        systemprompt_identifiers::Actor::user(UserId::new("declared-user")),
    )
    .with_user_type(kind)
}

async fn layers(rate_limits: RateLimitsConfig, caller: UserType) -> MountLayers {
    let mut config = fixture_config("postgres://localhost/declared");
    config.rate_limits = rate_limits;
    config.trusted_proxies = Vec::new();
    let db = test_db_pool().await;
    let extractor = StubExtractor {
        context: Some(context(caller)),
    };
    MountLayers {
        limits: RateLimitState::new(&config, Arc::new(UserRateLimitBucketRepository::new(&db))),
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(extractor.clone()),
        a2a: A2AContextMiddleware::new(extractor.clone(), 1024, BodyBudget::new(64 * 1024 * 1024)),
        mcp: McpContextMiddleware::new(extractor),
        bearer: None,
        guards: ProtectionGuards::default(),
    }
}

fn unlimited() -> RateLimitsConfig {
    RateLimitsConfig {
        disabled: true,
        ..RateLimitsConfig::default()
    }
}

fn ok_router() -> Router {
    Router::new().route("/", get(|| async { "ok" })).route(
        "/echo",
        post(|body: axum::body::Bytes| async move { body.len().to_string() }),
    )
}

fn request(caller: UserType) -> Request<Body> {
    request_with(caller, "/", Body::empty(), "GET")
}

fn request_with(caller: UserType, uri: &str, body: Body, method: &str) -> Request<Body> {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .body(body)
        .expect("test request must build");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4343))));
    req.extensions_mut().insert(context(caller));
    req
}

async fn status_for(protection: &RouteProtection, caller: UserType) -> StatusCode {
    let app = protect(
        ok_router(),
        protection,
        &layers(unlimited(), caller).await,
        BASE,
    )
    .expect("declaration must mount");
    app.oneshot(request(caller))
        .await
        .expect("request must complete")
        .status()
}

#[tokio::test]
async fn every_inventoried_policy_and_context_pairing_admits_and_refuses_as_declared() {
    let service_only = AuthzPolicy::restricted_to(&[UserType::Service]);
    let cases = [
        (
            AuthzPolicy::public(),
            ContextKind::Public,
            UserType::Anon,
            true,
        ),
        (AuthzPolicy::user(), ContextKind::User, UserType::User, true),
        (
            AuthzPolicy::user(),
            ContextKind::User,
            UserType::Anon,
            false,
        ),
        (
            AuthzPolicy::admin(),
            ContextKind::User,
            UserType::Admin,
            true,
        ),
        (
            AuthzPolicy::admin(),
            ContextKind::User,
            UserType::User,
            false,
        ),
        (
            AuthzPolicy::authenticated(),
            ContextKind::User,
            UserType::Service,
            true,
        ),
        (
            AuthzPolicy::authenticated(),
            ContextKind::User,
            UserType::Anon,
            false,
        ),
        (
            AuthzPolicy::authenticated(),
            ContextKind::A2a,
            UserType::A2a,
            true,
        ),
        (
            AuthzPolicy::authenticated(),
            ContextKind::A2a,
            UserType::Anon,
            false,
        ),
        (service_only, ContextKind::User, UserType::Service, true),
        (service_only, ContextKind::User, UserType::User, false),
        (
            AuthzPolicy::deferred_to_handler(),
            ContextKind::Mcp,
            UserType::Anon,
            true,
        ),
    ];
    for (policy, kind, caller, admitted) in cases {
        let status = status_for(&RouteProtection::new(policy, kind), caller).await;
        if admitted {
            assert_eq!(
                status,
                StatusCode::OK,
                "{policy:?}/{kind:?} must admit {caller:?}"
            );
        } else {
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{policy:?}/{kind:?} must refuse {caller:?}"
            );
        }
    }
}

#[tokio::test]
async fn a_policy_that_admits_no_caller_type_is_refused_at_mount() {
    let protection = RouteProtection::new(AuthzPolicy::restricted_to(&[]), ContextKind::User);
    let result = protect(
        ok_router(),
        &protection,
        &layers(unlimited(), UserType::User).await,
        BASE,
    );
    assert!(matches!(
        result,
        Err(MountError::NoAdmittedCaller { base_path: BASE })
    ));
}

#[tokio::test]
async fn a_bearer_window_without_an_extractor_is_refused_at_mount() {
    let protection = RouteProtection::new(AuthzPolicy::user(), ContextKind::User)
        .with_rate_limit(RateLimit::new("declared", RateBudget::PerSecond(5)).with_bearer_window());
    let limited = RateLimitsConfig {
        disabled: false,
        ..RateLimitsConfig::default()
    };
    let result = protect(
        ok_router(),
        &protection,
        &layers(limited, UserType::User).await,
        BASE,
    );
    assert!(matches!(
        result,
        Err(MountError::BearerWindowUnavailable { .. })
    ));
}

#[tokio::test]
async fn a_protection_layer_without_its_guard_is_refused_at_mount() {
    for layer in [
        ProtectionLayer::SiteAuth,
        ProtectionLayer::RefuseFileManagedClientWrites,
    ] {
        let protection =
            RouteProtection::new(AuthzPolicy::public(), ContextKind::Public).with_layer(layer);
        let result = protect(
            ok_router(),
            &protection,
            &layers(unlimited(), UserType::Anon).await,
            BASE,
        );
        assert!(
            matches!(result, Err(MountError::GuardUnavailable { layer: l, .. }) if l == layer),
            "{layer:?} without a guard must not mount"
        );
    }
}

#[tokio::test]
async fn the_declared_rate_limit_is_applied() {
    let protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
        .with_rate_limit(RateLimit::new("declared-rate", RateBudget::PerSecond(1)));
    let limited = RateLimitsConfig {
        disabled: false,
        burst_multiplier: 1,
        ..RateLimitsConfig::default()
    };
    let app = protect(
        ok_router(),
        &protection,
        &layers(limited, UserType::Anon).await,
        BASE,
    )
    .expect("declaration must mount");
    let mut statuses = Vec::new();
    for _ in 0..8 {
        statuses.push(
            app.clone()
                .oneshot(request(UserType::Anon))
                .await
                .expect("request must complete")
                .status(),
        );
    }
    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "a burst against a 1/s declared limit must be refused somewhere: {statuses:?}"
    );
}

#[tokio::test]
async fn the_declared_body_limit_is_applied() {
    let protection =
        RouteProtection::new(AuthzPolicy::public(), ContextKind::Public).with_body_limit(16);
    let app = protect(
        ok_router(),
        &protection,
        &layers(unlimited(), UserType::Anon).await,
        BASE,
    )
    .expect("declaration must mount");
    let resp = app
        .oneshot(request_with(
            UserType::Anon,
            "/echo",
            Body::from(vec![0_u8; 64]),
            "POST",
        ))
        .await
        .expect("request must complete");
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn the_declared_frame_options_are_stamped_on_the_response() {
    let protection = RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
        .with_frame_options(FrameOptions::AllowAll);
    let app = protect(
        ok_router(),
        &protection,
        &layers(unlimited(), UserType::Anon).await,
        BASE,
    )
    .expect("declaration must mount");
    let resp = app
        .oneshot(request(UserType::Anon))
        .await
        .expect("request must complete");
    let stamped = resp.extensions().get::<FrameOptionsOverride>().map(|o| o.0);
    assert_eq!(stamped, Some(FrameOptions::AllowAll));
}

#[tokio::test]
async fn a_public_context_with_a_policy_excluding_anonymous_callers_is_refused_at_mount() {
    for policy in [
        AuthzPolicy::user(),
        AuthzPolicy::admin(),
        AuthzPolicy::authenticated(),
    ] {
        let result = protect(
            ok_router(),
            &RouteProtection::new(policy, ContextKind::Public),
            &layers(unlimited(), UserType::Anon).await,
            BASE,
        );
        assert!(
            matches!(
                result,
                Err(MountError::IncoherentContext { base_path: BASE })
            ),
            "{policy:?} behind the public context must not mount"
        );
    }
}

#[test]
fn an_extension_body_limit_above_the_server_default_is_refused() {
    let at_ceiling =
        ExtensionRouter::new(Router::new(), BASE, AuthzPolicy::user(), ContextKind::User)
            .with_body_limit(DEFAULT_BODY_LIMIT_BYTES);
    assert!(admit_extension(&at_ceiling).is_ok());

    let above = ExtensionRouter::new(Router::new(), BASE, AuthzPolicy::user(), ContextKind::User)
        .with_body_limit(DEFAULT_BODY_LIMIT_BYTES + 1);
    assert!(matches!(
        admit_extension(&above),
        Err(MountError::ExtensionBodyLimitTooLarge {
            declared,
            ceiling: DEFAULT_BODY_LIMIT_BYTES,
            ..
        }) if declared == DEFAULT_BODY_LIMIT_BYTES + 1
    ));
}
