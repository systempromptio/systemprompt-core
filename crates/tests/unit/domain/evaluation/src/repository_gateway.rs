//! DB-backed tests for gateway admission of evaluation traffic: session
//! binding, reservation of bounded spend against the experiment budget, and
//! settlement once the provider has actually reported usage.

use crate::repository_workers::{Harness, MODEL, PROVIDER};
use systemprompt_evaluation::repository::experiments::{
    AdmissionRequest, ExecutionCapabilityRepository, ExecutionCompletion, ExecutionLease,
    GatewayEvaluationRepository, RequestAdmission, TerminalOutcome,
};
use systemprompt_identifiers::{ActorKind, AiRequestId, ModelId, ProviderId, SessionId, UserId};
use systemprompt_test_fixtures::seed_user_session;
use uuid::Uuid;

fn admission<'a>(
    harness: &'a Harness,
    session: &'a SessionId,
    request: &'a AiRequestId,
    model: &'a ModelId,
    provider: &'a ProviderId,
) -> AdmissionRequest<'a> {
    AdmissionRequest::builder(&harness.owner, session)
        .request(request)
        .model(model)
        .provider(provider)
        .bound_microdollars(50_000)
        .build()
        .expect("admission request")
}

#[tokio::test]
async fn bound_sessions_expose_an_evaluation_job_actor() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let gateway = GatewayEvaluationRepository::new(harness.pg.clone());

    let unbound = SessionId::generate();
    assert!(
        gateway
            .execution_actor(&harness.owner, &unbound)
            .await
            .expect("actor lookup")
            .is_none()
    );
    assert!(
        !gateway
            .is_evaluation_session(&unbound)
            .await
            .expect("lookup")
    );

    let access = ExecutionCapabilityRepository::new(harness.pg.clone())
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");

    assert!(
        gateway
            .is_evaluation_session(&access.session_id)
            .await
            .expect("lookup")
    );
    let actor = gateway
        .execution_actor(&harness.owner, &access.session_id)
        .await
        .expect("actor lookup")
        .expect("bound session has an actor");
    assert_eq!(actor.user_id, harness.owner);
    assert_eq!(
        actor.kind,
        ActorKind::Job {
            job_name: format!("evaluation:{}", execution.id.as_str())
        },
        "evaluation traffic is attributed to a job so sampling never grades it"
    );

    assert!(
        gateway
            .execution_actor(&UserId::new("someone-else"), &access.session_id)
            .await
            .expect("actor lookup")
            .is_none(),
        "the actor lookup is owner scoped"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn binding_a_session_requires_a_live_lease_and_is_single_flight() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (execution, lease) = harness.claimed_lease().await;
    let gateway = GatewayEvaluationRepository::new(harness.pg.clone());

    let session = SessionId::generate();
    seed_user_session(&harness.pool, &harness.owner, &session)
        .await
        .expect("seed session");

    let stale = ExecutionLease::builder(execution.id.clone(), harness.worker.id.clone())
        .fencing_token(lease.fencing_token + 1)
        .build()
        .expect("stale lease");
    assert!(
        gateway
            .bind_session(&harness.owner, &stale, &session)
            .await
            .is_err()
    );

    let unknown_session = SessionId::generate();
    assert!(
        gateway
            .bind_session(&harness.owner, &lease, &unknown_session)
            .await
            .is_err(),
        "the session must already exist for the owner"
    );

    gateway
        .bind_session(&harness.owner, &lease, &session)
        .await
        .expect("bind");
    assert!(
        gateway
            .bind_session(&harness.owner, &lease, &session)
            .await
            .is_err(),
        "a session binds to exactly one execution"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn admission_reserves_bounded_spend_and_settles_once_usage_lands() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let gateway = GatewayEvaluationRepository::new(harness.pg.clone());
    let access = ExecutionCapabilityRepository::new(harness.pg.clone())
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");

    let model = ModelId::new(MODEL);
    let provider = ProviderId::new(PROVIDER);
    let request = harness
        .seed_pending_request(access.session_id.as_str())
        .await;

    let admitted = gateway
        .admit(&admission(
            &harness,
            &access.session_id,
            &request,
            &model,
            &provider,
        ))
        .await
        .expect("admit");
    let RequestAdmission::Reserved(reservation) = admitted else {
        panic!("an evaluation session must be admitted against its budget");
    };
    assert_eq!(harness.budget().await, (50_000, 0));

    assert!(
        gateway
            .admit(&admission(
                &harness,
                &access.session_id,
                &request,
                &model,
                &provider
            ))
            .await
            .is_err(),
        "the same request is never dispatched twice"
    );

    assert!(
        gateway
            .settle_recorded(&harness.owner, &request)
            .await
            .is_err(),
        "a reservation stays held until the provider reports complete usage"
    );

    harness.complete_request(&request, 12_500).await;
    assert!(
        gateway
            .settle_recorded(&harness.owner, &request)
            .await
            .expect("settle")
    );
    assert_eq!(harness.budget().await, (0, 12_500));

    let settled = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT actual FROM eval_budget_reservations WHERE id = $1",
    )
    .bind(reservation.as_str())
    .fetch_one(&harness.pg)
    .await
    .expect("read reservation");
    assert_eq!(settled, Some(12_500));

    harness.cleanup().await;
}

#[tokio::test]
async fn ordinary_traffic_and_foreign_requests_are_not_reserved() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let gateway = GatewayEvaluationRepository::new(harness.pg.clone());
    let model = ModelId::new(MODEL);
    let provider = ProviderId::new(PROVIDER);

    let session = SessionId::generate();
    seed_user_session(&harness.pool, &harness.owner, &session)
        .await
        .expect("seed session");
    let request = harness.seed_pending_request(session.as_str()).await;

    let admitted = gateway
        .admit(&admission(&harness, &session, &request, &model, &provider))
        .await
        .expect("admit");
    assert_eq!(
        admitted,
        RequestAdmission::Ordinary,
        "traffic on an unbound session passes through unreserved"
    );
    assert_eq!(harness.budget().await, (0, 0));

    assert!(
        !gateway
            .settle_recorded(
                &harness.owner,
                &AiRequestId::new(format!("missing-{}", Uuid::new_v4()))
            )
            .await
            .expect("settle lookup"),
        "settlement of an unmapped request is a no-op, not an error"
    );

    harness.cleanup().await;
}

#[tokio::test]
async fn admission_rejects_a_mismatched_model_and_an_unaudited_request() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let gateway = GatewayEvaluationRepository::new(harness.pg.clone());
    let access = ExecutionCapabilityRepository::new(harness.pg.clone())
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");

    let model = ModelId::new(MODEL);
    let provider = ProviderId::new(PROVIDER);
    let request = harness
        .seed_pending_request(access.session_id.as_str())
        .await;

    assert!(
        gateway
            .admit(&admission(
                &harness,
                &access.session_id,
                &request,
                &ModelId::new("some-other-model"),
                &provider
            ))
            .await
            .is_err(),
        "a bound execution may only call the model its variant froze"
    );
    assert!(
        gateway
            .admit(&admission(
                &harness,
                &access.session_id,
                &request,
                &model,
                &ProviderId::new("some-other-provider")
            ))
            .await
            .is_err()
    );

    let unaudited = AiRequestId::new(format!("unaudited-{}", Uuid::new_v4()));
    assert!(
        gateway
            .admit(&admission(
                &harness,
                &access.session_id,
                &unaudited,
                &model,
                &provider
            ))
            .await
            .is_err(),
        "admission requires a pending audit record owned by the session"
    );
    assert_eq!(harness.budget().await, (0, 0));

    harness.cleanup().await;
}

#[tokio::test]
async fn admission_rejects_a_session_whose_execution_has_finished() {
    let Some(harness) = Harness::start().await else {
        return;
    };
    let (_, lease) = harness.claimed_lease().await;
    let gateway = GatewayEvaluationRepository::new(harness.pg.clone());
    let access = ExecutionCapabilityRepository::new(harness.pg.clone())
        .issue(&harness.owner, &lease)
        .await
        .expect("issue");
    let request = harness
        .seed_pending_request(access.session_id.as_str())
        .await;

    harness
        .experiments()
        .complete(
            &harness.owner,
            &lease,
            &ExecutionCompletion {
                outcome: TerminalOutcome::Completed,
                summary: "finished".to_owned(),
            },
        )
        .await
        .expect("complete");

    assert!(
        gateway
            .admit(&admission(
                &harness,
                &access.session_id,
                &request,
                &ModelId::new(MODEL),
                &ProviderId::new(PROVIDER)
            ))
            .await
            .is_err(),
        "a finished execution cannot keep spending"
    );
    assert_eq!(harness.budget().await, (0, 0));

    harness.cleanup().await;
}

#[test]
fn admission_request_builder_requires_every_dispatch_input() {
    let owner = UserId::new("owner");
    let session = SessionId::generate();
    let request = AiRequestId::new("request-1");
    let model = ModelId::new(MODEL);
    let provider = ProviderId::new(PROVIDER);

    let built = AdmissionRequest::builder(&owner, &session)
        .request(&request)
        .model(&model)
        .provider(&provider)
        .bound_microdollars(10)
        .build()
        .expect("built");
    assert_eq!(built.bound_microdollars, 10);
    assert_eq!(built.request, &request);

    assert!(
        AdmissionRequest::builder(&owner, &session)
            .model(&model)
            .provider(&provider)
            .bound_microdollars(10)
            .build()
            .is_err()
    );
    assert!(
        AdmissionRequest::builder(&owner, &session)
            .request(&request)
            .provider(&provider)
            .bound_microdollars(10)
            .build()
            .is_err()
    );
    assert!(
        AdmissionRequest::builder(&owner, &session)
            .request(&request)
            .model(&model)
            .bound_microdollars(10)
            .build()
            .is_err()
    );
    assert!(
        AdmissionRequest::builder(&owner, &session)
            .request(&request)
            .model(&model)
            .provider(&provider)
            .build()
            .is_err()
    );
    assert!(
        AdmissionRequest::builder(&owner, &session)
            .request(&request)
            .model(&model)
            .provider(&provider)
            .bound_microdollars(0)
            .build()
            .is_err()
    );
}
