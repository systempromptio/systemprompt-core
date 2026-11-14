//! Handler-verified route declarations. A handler-verified policy mounts with
//! no context middleware and no gate, so a request carrying no
//! `RequestContext` reaches the handler, while the rate limit it declares is
//! still applied; it pairs only with the handler-resolved context, the
//! handler-resolved context pairs only with it, an extension may declare
//! neither, and a core body limit above the server default must be a listed
//! exemption named for that group's base path.

use std::net::SocketAddr;

use axum::Router;
use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::StatusCode;
use systemprompt_api::services::middleware::{
    CoreBodyExemption, DEFAULT_BODY_LIMIT_BYTES, MountError, admit_extension, protect,
};
use systemprompt_extension::{
    AuthzPolicy, ContextKind, ExtensionRouter, HandlerVerifier, RateBudget, RateLimit,
    RouteProtection,
};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::auth::UserType;
use tower::ServiceExt;

use super::declared_mount::{BASE, layers, ok_router, unlimited};

fn handler_verified(verifier: HandlerVerifier) -> RouteProtection {
    RouteProtection::new(
        AuthzPolicy::handler_verified(verifier),
        ContextKind::HandlerResolved,
    )
}

fn bare_request() -> Request<Body> {
    let mut req = Request::builder()
        .uri("/")
        .body(Body::empty())
        .expect("test request must build");
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 4343))));
    req
}

#[tokio::test]
async fn a_handler_verified_group_reaches_its_handler_without_a_request_context() {
    let app = protect(
        ok_router(),
        &handler_verified(HandlerVerifier::SlackSignature),
        &layers(unlimited(), UserType::Anon).await,
        BASE,
    )
    .expect("declaration must mount");
    let status = app
        .oneshot(bare_request())
        .await
        .expect("request must complete")
        .status();
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_context_middleware_refuses_the_same_bare_request() {
    let app = protect(
        ok_router(),
        &RouteProtection::new(AuthzPolicy::public(), ContextKind::Public),
        &layers(unlimited(), UserType::Anon).await,
        BASE,
    )
    .expect("declaration must mount");
    let status = app
        .oneshot(bare_request())
        .await
        .expect("request must complete")
        .status();
    assert_ne!(status, StatusCode::OK);
}

#[tokio::test]
async fn a_handler_verified_group_keeps_its_declared_rate_limit() {
    let protection = handler_verified(HandlerVerifier::GatewayCredential)
        .with_rate_limit(RateLimit::new("handler-verified", RateBudget::PerSecond(1)));
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
                .oneshot(bare_request())
                .await
                .expect("request must complete")
                .status(),
        );
    }
    assert!(
        statuses.contains(&StatusCode::TOO_MANY_REQUESTS),
        "a burst against a 1/s handler-verified limit must be refused somewhere: {statuses:?}"
    );
}

#[tokio::test]
async fn handler_verification_and_the_handler_resolved_context_pair_only_with_each_other() {
    let incoherent = [
        RouteProtection::new(
            AuthzPolicy::handler_verified(HandlerVerifier::McpHopToken),
            ContextKind::User,
        ),
        RouteProtection::new(
            AuthzPolicy::handler_verified(HandlerVerifier::McpHopToken),
            ContextKind::Public,
        ),
        RouteProtection::new(AuthzPolicy::public(), ContextKind::HandlerResolved),
        RouteProtection::new(AuthzPolicy::user(), ContextKind::HandlerResolved),
    ];
    for protection in incoherent {
        let result = protect(
            ok_router(),
            &protection,
            &layers(unlimited(), UserType::Anon).await,
            BASE,
        );
        assert!(
            matches!(
                result,
                Err(MountError::IncoherentHandlerVerification { base_path: BASE })
            ),
            "{protection:?} must not mount"
        );
    }
}

#[test]
fn an_extension_may_not_declare_handler_verification() {
    let declarations = [
        ExtensionRouter::new(
            Router::new(),
            BASE,
            AuthzPolicy::handler_verified(HandlerVerifier::SlackSignature),
            ContextKind::HandlerResolved,
        ),
        ExtensionRouter::new(
            Router::new(),
            BASE,
            AuthzPolicy::public(),
            ContextKind::HandlerResolved,
        ),
    ];
    for declared in declarations {
        assert!(matches!(
            admit_extension(&declared),
            Err(MountError::ExtensionHandlerVerified { base_path: BASE })
        ));
    }
}

#[tokio::test]
async fn a_core_body_limit_above_the_default_must_be_exempted_for_its_own_group() {
    let unlisted = RouteProtection::new(AuthzPolicy::authenticated(), ContextKind::User)
        .with_body_limit(DEFAULT_BODY_LIMIT_BYTES + 1);
    let result = protect(
        ok_router(),
        &unlisted,
        &layers(unlimited(), UserType::User).await,
        BASE,
    );
    assert!(matches!(
        result,
        Err(MountError::UnlistedBodyExemption { declared, .. })
            if declared == DEFAULT_BODY_LIMIT_BYTES + 1
    ));

    for exemption in CoreBodyExemption::ALL {
        let listed = RouteProtection::new(AuthzPolicy::authenticated(), ContextKind::User)
            .with_body_limit(exemption.bytes());
        assert!(
            protect(
                ok_router(),
                &listed,
                &layers(unlimited(), UserType::User).await,
                exemption.base_path(),
            )
            .is_ok(),
            "{exemption:?} must mount at its own base path"
        );
        assert!(
            matches!(
                protect(
                    ok_router(),
                    &listed,
                    &layers(unlimited(), UserType::User).await,
                    BASE,
                ),
                Err(MountError::UnlistedBodyExemption { base_path: BASE, .. })
            ),
            "{exemption:?}'s limit must not mount under another group"
        );
    }
}
