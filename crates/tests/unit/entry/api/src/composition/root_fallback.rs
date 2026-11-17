//! The web root's one fallback: a root group's own fallback is discarded, so
//! an unmatched path never meets that group's gate; the fallback owner is
//! layered whole; a second claim on the fallback is refused.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use systemprompt_api::services::middleware::{
    A2AContextMiddleware, BodyBudget, ContextExtractor, McpContextMiddleware, MountError,
    MountLayers, ProtectionGuards, PublicContextMiddleware, RateLimitState, RootFallbackOwner,
    UserOnlyContextMiddleware, protect, protect_root_fallback,
};
use systemprompt_extension::reserved::WEB_ROOT_BASE_PATH;
use systemprompt_extension::{
    AuthzPolicy, ContextKind, HandlerVerifier, RouteGroup, RouteProtection,
};
use systemprompt_models::RequestContext;
use systemprompt_models::execution::ContextExtractionError;
use systemprompt_test_fixtures::{fixture_config, test_db_pool};
use systemprompt_users::UserRateLimitBucketRepository;
use tower::ServiceExt;

use super::forge::ok;

#[derive(Clone)]
struct NoContext;

#[async_trait::async_trait]
impl ContextExtractor for NoContext {
    async fn extract_from_headers(
        &self,
        _headers: &HeaderMap,
    ) -> Result<RequestContext, ContextExtractionError> {
        Err(ContextExtractionError::MissingAuthHeader)
    }
}

async fn layers() -> MountLayers {
    let config = fixture_config("postgres://localhost/root-fallback");
    let db = test_db_pool().await;
    MountLayers {
        limits: RateLimitState::new(&config, Arc::new(UserRateLimitBucketRepository::new(&db))),
        public: PublicContextMiddleware::new(),
        user: UserOnlyContextMiddleware::new(NoContext),
        a2a: A2AContextMiddleware::new(NoContext, 1024, BodyBudget::new(64 * 1024 * 1024)),
        mcp: McpContextMiddleware::new(NoContext),
        bearer: None,
        guards: ProtectionGuards::default(),
    }
}

async fn status_at(router: Router, uri: &str) -> StatusCode {
    let mut request = Request::get(uri)
        .body(Body::empty())
        .expect("valid request");
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4000))));
    router.oneshot(request).await.expect("infallible").status()
}

async fn teapot() -> StatusCode {
    StatusCode::IM_A_TEAPOT
}

fn user_gated() -> RouteProtection {
    RouteProtection::new(AuthzPolicy::user(), ContextKind::User)
}

fn public() -> RouteProtection {
    RouteProtection::new(AuthzPolicy::public(), ContextKind::Public)
}

fn ungated() -> RouteProtection {
    RouteProtection::new(
        AuthzPolicy::handler_verified(HandlerVerifier::GatewayCredential),
        ContextKind::HandlerResolved,
    )
}

fn with_fallback() -> Router {
    Router::new().route("/dash", get(ok)).fallback(teapot)
}

#[tokio::test]
async fn a_root_group_fallback_is_discarded_not_gated() {
    let layers = layers().await;
    let router = protect(with_fallback(), &user_gated(), &layers, WEB_ROOT_BASE_PATH)
        .expect("a root group mounts");
    let gated = status_at(router.clone(), "/dash").await;
    assert_ne!(gated, StatusCode::OK, "the route is gated");
    assert_eq!(
        status_at(router, "/elsewhere").await,
        StatusCode::NOT_FOUND,
        "an unmatched path is neither the group's gate nor its fallback"
    );
}

#[tokio::test]
async fn the_fallback_owner_is_layered_whole() {
    let layers = layers().await;
    let owned = protect_root_fallback(with_fallback(), &user_gated(), &layers, RouteGroup::Static)
        .expect("the owner mounts");
    let router = RootFallbackOwner::default()
        .place(Router::new(), owned)
        .expect("one owner is placed");
    let fallback = status_at(router, "/elsewhere").await;
    assert_ne!(
        fallback,
        StatusCode::IM_A_TEAPOT,
        "the gate covers the fallback"
    );
    assert_ne!(
        fallback,
        StatusCode::NOT_FOUND,
        "the owner's fallback is the tree's"
    );

    let open = protect_root_fallback(with_fallback(), &ungated(), &layers, RouteGroup::Static)
        .expect("an ungated owner mounts");
    let router = RootFallbackOwner::default()
        .place(Router::new(), open)
        .expect("one owner is placed");
    assert_eq!(
        status_at(router, "/elsewhere").await,
        StatusCode::IM_A_TEAPOT,
        "negative control: an ungated owner's fallback answers"
    );
}

#[tokio::test]
async fn a_second_fallback_owner_is_refused() {
    let layers = layers().await;
    let mut owner = RootFallbackOwner::default();
    let first = protect_root_fallback(with_fallback(), &public(), &layers, RouteGroup::Static)
        .expect("the first owner mounts");
    let tree = owner
        .place(Router::new(), first)
        .expect("the first claim is placed");
    let second = protect_root_fallback(
        Router::new().fallback(teapot),
        &public(),
        &layers,
        RouteGroup::Extensions,
    )
    .expect("the second router protects");
    assert!(matches!(
        owner.place(tree, second),
        Err(MountError::SecondRootFallback {
            held: RouteGroup::Static,
            claimed: RouteGroup::Extensions,
        })
    ));
}
