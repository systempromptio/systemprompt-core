//! Transport-level admission for the evaluator worker endpoints.
//!
//! Every route on this router mutates execution state, so identity is
//! server-owned: the bearer token must resolve to an enabled worker in this
//! environment, and the lease in the body must belong to that same worker.
//! These tests drive the real router against a migrated database with no
//! execution rows, which is exactly the state in which a forged or replayed
//! request arrives.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use systemprompt_evaluation::repository::experiments::{ExecutionLease, WorkerRepository};
use systemprompt_identifiers::{EvalExecutionId, EvalWorkerId, UserId};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool, seed_user_row};
use tower::ServiceExt;

const ENVIRONMENT: &str = "https://evaluation.test.invalid";

struct Harness {
    router: Router,
    pool: sqlx::PgPool,
    owner: UserId,
}

async fn harness() -> Option<Harness> {
    let url = fixture_database_url().ok()?;
    let db_pool = fixture_db_pool(&url).await.ok()?;
    let owner = UserId::new(format!("eval-transport-{}", EvalWorkerId::generate()));
    seed_user_row(&db_pool, &owner, &format!("{owner}@transport.test.invalid"))
        .await
        .ok()?;
    let pool = (*db_pool.write_pool_arc().ok()?).clone();
    let state = systemprompt_api::routes::evaluation::EvaluationWorkerState::builder(pool.clone())
        .environment(ENVIRONMENT.to_owned())
        .build()
        .ok()?;
    Some(Harness {
        router: systemprompt_api::routes::evaluation::router(state),
        pool,
        owner,
    })
}

async fn call(router: &Router, path: &str, authorization: Option<&str>, body: &str) -> StatusCode {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json");
    if let Some(value) = authorization {
        request = request.header("authorization", value);
    }
    router
        .clone()
        .oneshot(request.body(Body::from(body.to_owned())).expect("request"))
        .await
        .expect("the router must answer every request")
        .status()
}

fn execution_of(lease: &str) -> String {
    serde_json::from_str::<serde_json::Value>(lease)
        .ok()
        .and_then(|value| value["execution_id"].as_str().map(str::to_owned))
        .expect("the serialised lease carries its execution id")
}

fn body_for(path: &str, lease: &str) -> String {
    match path {
        "/events" => format!(
            r#"{{"lease":{lease},"event":{{"sequence":1,"stage":"provisioning","summary":"container provisioned"}}}}"#
        ),
        "/evidence" => format!(
            r#"{{"lease":{lease},"evidence":{{"execution_id":"{execution}","fencing_token":1,"capabilities":{{"client":"claude-code","client_version":"1.0.0","adapter_version":"1.0.0","image_digest":"sha256:{digest}","supports_session_resume":false}},"installed_bundle_digest":"sha256:{digest}","candidate_bundle_digest":"sha256:{digest}","workspace_digest":"sha256:{digest}","requests":[],"artifacts":[],"exit_code":0,"elapsed_milliseconds":1200,"cleanup_confirmed":true}},"artifacts":{{"files":{{}}}}}}"#,
            execution = execution_of(lease),
            digest = "0".repeat(64),
        ),
        "/complete" => format!(
            r#"{{"lease":{lease},"completion":{{"outcome":"completed","summary":"finished"}}}}"#
        ),
        _ => lease.to_owned(),
    }
}

fn lease(worker: &EvalWorkerId) -> String {
    let lease = ExecutionLease::builder(EvalExecutionId::generate(), worker.clone())
        .fencing_token(1)
        .build()
        .expect("a fenced lease must build");
    serde_json::to_string(&lease).expect("lease serialises")
}

const MUTATING_ROUTES: [&str; 6] = [
    "/assignment",
    "/events",
    "/access",
    "/heartbeat",
    "/evidence",
    "/complete",
];

#[tokio::test]
async fn a_request_without_a_bearer_token_is_rejected() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };

    let anonymous = lease(&EvalWorkerId::generate());
    for path in std::iter::once("/claim").chain(MUTATING_ROUTES) {
        assert_eq!(
            call(&harness.router, path, None, &body_for(path, &anonymous)).await,
            StatusCode::UNAUTHORIZED,
            "{path} must refuse an unauthenticated caller before parsing any body"
        );
    }
}

#[tokio::test]
async fn malformed_and_unknown_credentials_are_rejected() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };

    for header in [
        "speval_no-bearer-scheme",
        "Basic c3BldmFsOnNlY3JldA==",
        "Bearer ",
        "Bearer not-an-evaluator-token",
        &format!("Bearer speval_{}", "a".repeat(200)),
        "Bearer speval_unknown.credential",
    ] {
        assert_eq!(
            call(&harness.router, "/claim", Some(header), "{}").await,
            StatusCode::UNAUTHORIZED,
            "credential {header:?} must not authenticate a worker"
        );
    }
}

#[tokio::test]
async fn a_credential_from_another_environment_is_rejected() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };
    let credential = WorkerRepository::new(harness.pool.clone())
        .create(
            &harness.owner,
            "https://other.test.invalid",
            "foreign-worker",
        )
        .await
        .expect("a worker credential must be issuable");

    let status = call(
        &harness.router,
        "/claim",
        Some(&format!("Bearer {}", credential.expose_token())),
        "{}",
    )
    .await;

    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a credential minted for another environment must not authenticate here"
    );
}

#[tokio::test]
async fn an_authenticated_worker_claims_nothing_when_no_work_is_queued() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };
    let credential = WorkerRepository::new(harness.pool.clone())
        .create(&harness.owner, ENVIRONMENT, "idle-worker")
        .await
        .expect("a worker credential must be issuable");

    let status = call(
        &harness.router,
        "/claim",
        Some(&format!("Bearer {}", credential.expose_token())),
        "{}",
    )
    .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "an enabled worker in this environment must be allowed to poll for work"
    );
}

#[tokio::test]
async fn a_lease_belonging_to_another_worker_is_rejected() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };
    let workers = WorkerRepository::new(harness.pool.clone());
    let owner = harness.owner.clone();
    let caller = workers
        .create(&owner, ENVIRONMENT, "caller-worker")
        .await
        .expect("a worker credential must be issuable");
    let other = workers
        .create(&owner, ENVIRONMENT, "other-worker")
        .await
        .expect("a second worker credential must be issuable");
    let authorization = format!("Bearer {}", caller.expose_token());
    let foreign = lease(&other.id);

    for path in MUTATING_ROUTES {
        let body = body_for(path, &foreign);
        assert_eq!(
            call(&harness.router, path, Some(&authorization), &body).await,
            StatusCode::UNAUTHORIZED,
            "{path} must refuse a lease fenced to a different worker"
        );
    }
}

#[tokio::test]
async fn worker_routes_are_post_only() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };

    let response = harness
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/claim")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("the router must answer");

    assert_eq!(
        response.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "state-mutating worker endpoints must not be reachable by GET"
    );
}

#[tokio::test]
async fn an_unknown_execution_is_refused_for_its_own_worker() {
    // skip-ok: no migrated database is reachable from this environment
    let Some(harness) = harness().await else {
        return;
    };
    let credential = WorkerRepository::new(harness.pool.clone())
        .create(&harness.owner, ENVIRONMENT, "fenced-worker")
        .await
        .expect("a worker credential must be issuable");
    let authorization = format!("Bearer {}", credential.expose_token());
    let own = lease(&credential.id);

    for path in MUTATING_ROUTES {
        let status = call(&harness.router, path, Some(&authorization), &body_for(path, &own)).await;
        assert!(
            status.is_client_error(),
            "{path} must refuse a lease for an execution that was never handed out, got {status}"
        );
        assert_ne!(
            status,
            StatusCode::UNAUTHORIZED,
            "{path} authenticated the worker, so the refusal must name the missing execution \
             rather than the credential"
        );
    }
}
