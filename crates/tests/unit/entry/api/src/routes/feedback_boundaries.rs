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
        AgentName::try_new("feedback-boundaries").expect("valid fixture agent name"),
    )
    .with_actor(Actor::user(UserId::new(format!(
        "http-{}",
        TraceId::generate()
    ))));
    let origin = url::Url::parse(&ctx.config().api_external_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let consumer = systemprompt_api::routes::managed::consumer::router()
        .layer(axum::middleware::from_fn_with_state(
            ctx.as_ref().clone(),
            systemprompt_api::routes::managed::origin::protect,
        ))
        .with_state(ctx.as_ref().clone())
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));
    let admin = systemprompt_api::routes::managed::router()
        .with_state(
            systemprompt_api::routes::managed::state::ManagedState::new(
                ctx.as_ref().clone(),
            ),
        )
        .layer(Extension(actor))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
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
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let problem: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(problem["type"], "about:blank");
        assert!(problem["detail"].is_string());
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
async fn missing_job_is_not_null_success_and_errors_are_problem_details() {
    let (_, admin, _) = routers().await;
    for (uri, status) in [("/analytics/jobs/absent-job", StatusCode::NOT_FOUND)] {
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
            systemprompt_api::routes::managed::contract::normalize,
        ));
    let actor = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("consumer-policy").expect("valid fixture agent name"),
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

#[tokio::test]
async fn failed_capture_has_durable_status_and_conflicting_http_retry_is_rejected() {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let ctx = fixture_app_context(&db, &bootstrap.database_url).unwrap();
    systemprompt_test_fixtures::seed_user_row(
        &db,
        ctx.system_admin().id(),
        &format!("{}@api-operation-owner.invalid", ctx.system_admin().id()),
    )
    .await
    .unwrap();
    let router = systemprompt_api::routes::managed::router()
        .with_state(
            systemprompt_api::routes::managed::state::ManagedState::new(
                ctx.as_ref().clone(),
            ),
        )
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));
    let key = systemprompt_identifiers::TaskId::generate();
    let uri = format!(
        "/sources/{}/captures",
        systemprompt_identifiers::ManagedSourceId::generate()
    );
    let request = |skill: &str| {
        Request::builder()
            .method("POST")
            .uri(&uri)
            .header("content-type", "application/json")
            .header("idempotency-key", key.as_str())
            .body(Body::from(
                serde_json::json!({"skill_ids":[skill]}).to_string(),
            ))
            .unwrap()
    };
    let first = router.clone().oneshot(request("missing")).await.unwrap();
    assert_eq!(first.status(), StatusCode::NOT_FOUND);
    let status_uri = format!("/operations/{key}");
    let status = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&status_uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let body = axum::body::to_bytes(status.into_body(), 16384)
        .await
        .unwrap();
    let retained: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(retained["operation"]["state"], "failed");
    assert!(
        retained["operation"]["problem"]
            .as_str()
            .unwrap()
            .contains("new operation key")
    );
    assert!(retained["result"].is_null());
    let retry = router.clone().oneshot(request("missing")).await.unwrap();
    assert_eq!(retry.status(), StatusCode::OK);
    assert_eq!(retry.headers()["location"], format!("/api/v1{status_uri}"));
    let body = axum::body::to_bytes(retry.into_body(), 16384)
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
        retained
    );
    let conflict = router.oneshot(request("different")).await.unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        conflict.headers()["content-type"],
        "application/problem+json"
    );
}

