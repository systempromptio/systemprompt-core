//! Real feedback routers enforce device identity, origin, status and cursor
//! bounds.
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::{Extension, Router};
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::RequestContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use tower::ServiceExt;
async fn routers() -> (Router, Router, String) {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let ctx = fixture_app_context(&db, &bootstrap.database_url).unwrap();
    let actor = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("feedback-boundaries"),
    )
    .with_actor(Actor::user(UserId::new(format!(
        "http-{}",
        TraceId::generate()
    ))));
    let origin = url::Url::parse(&ctx.config().api_external_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let consumer = systemprompt_api::routes::evaluation::consumer::router()
        .layer(axum::middleware::from_fn_with_state(
            ctx.as_ref().clone(),
            systemprompt_api::routes::evaluation::optimization_origin::protect,
        ))
        .with_state(ctx.as_ref().clone())
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::evaluation::contract::normalize,
        ));
    let admin = systemprompt_api::routes::evaluation::campaigns::router()
        .with_state(ctx.as_ref().clone())
        .layer(Extension(actor))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::evaluation::contract::normalize,
        ));
    (consumer, admin, origin)
}
#[tokio::test]
async fn submitted_identity_user_secret_and_unenrolled_credentials_do_not_authenticate() {
    let (consumer, _, origin) = routers().await;
    for token in ["user-bridge-secret", "eyJ.user.jwt", "sp_device_forged"] {
        let response = consumer
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/consumer-devices/enrollment")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"device_id":"forged","consumer_id":"publisher"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
    }
    for supplied in [
        None,
        Some("https://untrusted.invalid"),
        Some(origin.as_str()),
    ] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/consumer-devices/enrollment")
            .header("cookie", "access_token=unused");
        if let Some(origin) = supplied {
            request = request.header("origin", origin);
        }
        let response = consumer
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if supplied == Some(origin.as_str()) {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::FORBIDDEN
            }
        );
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
    }
}
#[tokio::test]
async fn missing_job_is_not_null_success_and_collection_limits_are_enforced() {
    let (_, admin, _) = routers().await;
    for (uri, status) in [
        ("/analytics/jobs/absent-job", StatusCode::NOT_FOUND),
        ("/experiments?limit=0", StatusCode::BAD_REQUEST),
        ("/experiments?limit=101", StatusCode::BAD_REQUEST),
        ("/evaluator-capabilities?limit=101", StatusCode::BAD_REQUEST),
    ] {
        let response = admin
            .clone()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{uri}");
        assert_eq!(
            response.headers()["content-type"],
            "application/problem+json"
        );
    }
    let response = admin
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn consumer_identity_does_not_confer_administrative_access() {
    use systemprompt_api::services::middleware::{AuthzPolicy, PublicContextMiddleware, RouterExt};
    use systemprompt_models::auth::UserType;
    let (_, admin, _) = routers().await;
    let protected = admin
        .with_auth(PublicContextMiddleware::new(), AuthzPolicy::admin())
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::evaluation::contract::normalize,
        ));
    let actor = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("consumer-policy"),
    )
    .with_user_type(UserType::User)
    .with_actor(Actor::user(UserId::new("consumer-policy")));
    let mut request = Request::builder()
        .uri("/openapi.json")
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(actor);
    let response = protected.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response.headers()["content-type"],
        "application/problem+json"
    );
}
