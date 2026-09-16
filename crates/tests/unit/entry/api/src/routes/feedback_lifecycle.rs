//! Lifecycle HTTP decisions preserve admin policy, browser origin and retry
//! status.
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use axum::{Router, middleware};
use systemprompt_api::services::middleware::{AuthzPolicy, authz_gate};
use systemprompt_identifiers::{
    Actor, AgentName, ContextId, EvalApprovalId, EvalExecutionId, EvalExperimentId, EvalRevisionId,
    SessionId, TraceId,
};
use systemprompt_models::RequestContext;
use systemprompt_models::auth::UserType;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_db_pool, seed_user_row,
};
use tower::ServiceExt;

#[tokio::test]
async fn approval_http_requires_admin_origin_and_recovers_exact_decision_status() {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url).await.unwrap();
    let ctx = fixture_app_context(&db, &bootstrap.database_url).unwrap();
    let owner = ctx.system_admin().id();
    seed_user_row(&db, owner, &format!("{owner}@lifecycle.invalid"))
        .await
        .unwrap();
    let pool = db.write_pool_arc().unwrap();
    let budget = ctx
        .evaluation_repositories()
        .budgets
        .create_shared(owner, &format!("http-{}", TraceId::generate()), 100)
        .await
        .unwrap();
    let experiment = EvalExperimentId::generate();
    let case = EvalRevisionId::generate();
    let execution = EvalExecutionId::generate();
    let approval = EvalApprovalId::generate();
    sqlx::query("INSERT INTO eval_resource_revisions(id,owner_id,resource_kind,resource_key,digest,content) VALUES($1,$2,'case',$1,$3,'{}')").bind(case.as_str()).bind(owner.as_str()).bind("a".repeat(64)).execute(pool.as_ref()).await.unwrap();
    sqlx::query("INSERT INTO eval_experiments(id,owner_id,spec,spec_digest,budget_id,idempotency_key,status) VALUES($1,$2,'{}',$3,$4,$1,'running')").bind(experiment.as_str()).bind(owner.as_str()).bind("b".repeat(64)).bind(budget.as_str()).execute(pool.as_ref()).await.unwrap();
    sqlx::query("INSERT INTO eval_executions(id,experiment_id,variant_index,case_revision_id,repetition,status,fencing_token) VALUES($1,$2,0,$3,0,'awaiting_approval',1)").bind(execution.as_str()).bind(experiment.as_str()).bind(case.as_str()).execute(pool.as_ref()).await.unwrap();
    sqlx::query("INSERT INTO eval_execution_approvals(id,execution_id,owner_id,fencing_token,operation,operation_digest,precondition_digest) VALUES($1,$2,$3,1,'{}',$4,$4)").bind(approval.as_str()).bind(execution.as_str()).bind(owner.as_str()).bind("a".repeat(64)).execute(pool.as_ref()).await.unwrap();
    let router: Router = systemprompt_api::routes::evaluation::campaigns::router()
        .layer(middleware::from_fn_with_state(
            ctx.as_ref().clone(),
            systemprompt_api::routes::evaluation::optimization_origin::protect,
        ))
        .with_state(
            systemprompt_api::routes::evaluation::optimization_state::OptimizationState::new(
                ctx.as_ref().clone(),
            ),
        )
        .layer(middleware::from_fn(|request, next| {
            authz_gate(AuthzPolicy::admin(), request, next)
        }))
        .layer(middleware::from_fn(
            systemprompt_api::routes::evaluation::contract::normalize,
        ));
    let actor = |kind| {
        RequestContext::new(
            SessionId::generate(),
            TraceId::generate(),
            ContextId::generate(),
            AgentName::try_new("approval-http").unwrap(),
        )
        .with_user_type(kind)
        .with_actor(Actor::user(owner.clone()))
    };
    let key = format!("approval-{}", TraceId::generate());
    let path = format!("/evaluation-approvals/{approval}/decisions");
    let origin = url::Url::parse(&ctx.config().api_external_url)
        .unwrap()
        .origin()
        .ascii_serialization();
    let request = |kind, source: &str, approve| {
        let mut request=Request::builder().method("POST").uri(&path).header("content-type","application/json").header("cookie","session=fixture").header("origin",source).header("idempotency-key",&key).body(Body::from(serde_json::json!({"approve":approve,"observed_precondition_digest":"a".repeat(64)}).to_string())).unwrap();
        request.extensions_mut().insert(actor(kind));
        request
    };
    assert_eq!(
        router
            .clone()
            .oneshot(request(UserType::User, &origin, true))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(UserType::Admin, "https://foreign.invalid", true))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    for _ in 0..2 {
        let response = router
            .clone()
            .oneshot(request(UserType::Admin, &origin, true))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["location"],
            format!("/api/v1/operations/{key}")
        );
        let value: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 16384).await.unwrap()).unwrap();
        assert_eq!(value["result"]["status"], "approved");
        assert_eq!(value["operation"]["state"], "completed");
    }
    assert_eq!(
        router
            .clone()
            .oneshot(request(UserType::Admin, &origin, false))
            .await
            .unwrap()
            .status(),
        StatusCode::CONFLICT
    );
    for path in [
        format!("/evaluation-approvals/{approval}"),
        format!("/operations/{key}"),
    ] {
        let mut get = Request::builder().uri(path).body(Body::empty()).unwrap();
        get.extensions_mut().insert(actor(UserType::Admin));
        assert_eq!(
            router.clone().oneshot(get).await.unwrap().status(),
            StatusCode::OK
        );
    }
    let state: String = sqlx::query_scalar("SELECT status FROM eval_executions WHERE id=$1")
        .bind(execution.as_str())
        .fetch_one(pool.as_ref())
        .await
        .unwrap();
    assert_eq!(state, "queued");
    let document = systemprompt_api::routes::evaluation::contract::openapi::document();
    for (path, method) in [
        ("/evaluation-suggestions", "post"),
        ("/evaluation-suggestions/{id}", "get"),
        ("/evaluation-approvals/{id}", "get"),
        ("/evaluation-approvals/{id}/decisions", "post"),
    ] {
        assert!(document["paths"][path][method].is_object(), "{path}");
    }
    for (statement, id) in [
        (
            "DELETE FROM managed_api_operations WHERE id=$1",
            key.as_str(),
        ),
        (
            "DELETE FROM eval_execution_approvals WHERE id=$1",
            approval.as_str(),
        ),
        (
            "DELETE FROM eval_executions WHERE id=$1",
            execution.as_str(),
        ),
        (
            "DELETE FROM eval_experiments WHERE id=$1",
            experiment.as_str(),
        ),
        (
            "DELETE FROM eval_resource_revisions WHERE id=$1",
            case.as_str(),
        ),
        (
            "DELETE FROM eval_budget_accounts WHERE id=$1",
            budget.as_str(),
        ),
    ] {
        sqlx::query(statement)
            .bind(id)
            .execute(pool.as_ref())
            .await
            .unwrap();
    }
}
