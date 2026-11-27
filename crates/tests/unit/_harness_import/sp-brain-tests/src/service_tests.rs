//! The service follows the applied instance, reads whether its brain is
//! enabled, and fails closed until it is.

use serde_json::json;
use sp_brain::{BrainError, BrainService, BrainStatus, FactQuery};
use sp_protocol::wire::brain::BrainFactId;

use crate::support::{
    FACT, FakeResponse, Rig, error_json, fact_json, http, listing, revision_json,
};

fn fact() -> BrainFactId {
    BrainFactId::try_new(FACT).unwrap()
}

#[tokio::test]
async fn a_service_with_no_instance_refuses_without_a_request() {
    let service = BrainService::new(http());
    assert_eq!(service.status(), BrainStatus::Detached);
    let error = service.facts(&FactQuery::default()).await.unwrap_err();
    assert!(matches!(error, BrainError::NoInstance));
}

#[tokio::test]
async fn an_instance_whose_brain_answers_is_enabled() {
    let rig = Rig::start(|request| {
        listing(request)
            .unwrap_or_else(|| FakeResponse::json(200, &revision_json("confirmed", "confirmed")))
    })
    .await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    let probe = rig.fake.last();
    assert_eq!(probe.path(), "/api/v1/brain/facts");
    assert_eq!(probe.query(), "limit=1");
    let before = rig.service.feed();
    let revision = rig.service.confirm(&fact()).await.unwrap();
    assert_eq!(revision.revision, 4);
    let after = rig.service.feed();
    assert_eq!(after.facts, before.facts + 1);
    assert_eq!(after.proposals, before.proposals);
}

#[tokio::test]
async fn a_disabled_brain_is_withheld_and_never_asked_again() {
    let rig = Rig::start(|_| FakeResponse::json(404, &error_json("brain_disabled"))).await;
    assert_eq!(rig.settled().await, BrainStatus::Disabled);
    let asked = rig.fake.requests().len();
    let error = rig.service.fact(&fact()).await.unwrap_err();
    assert!(matches!(error, BrainError::Disabled));
    assert_eq!(rig.fake.requests().len(), asked);
}

#[tokio::test]
async fn an_answer_that_the_brain_is_disabled_withholds_it_from_then_on() {
    let rig = Rig::start(|request| {
        listing(request).unwrap_or_else(|| FakeResponse::json(404, &error_json("brain_disabled")))
    })
    .await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    assert!(matches!(
        rig.service.fact(&fact()).await,
        Err(BrainError::Disabled)
    ));
    assert_eq!(rig.service.status(), BrainStatus::Disabled);
}

#[tokio::test]
async fn clearing_the_instance_detaches_the_brain() {
    let rig = Rig::start(|request| {
        listing(request).unwrap_or_else(|| FakeResponse::json(200, &fact_json(FACT, "confirmed")))
    })
    .await;
    assert_eq!(rig.settled().await, BrainStatus::Enabled);
    assert_eq!(rig.service.fact(&fact()).await.unwrap().id, fact());
    rig.applier.clear();
    assert_eq!(rig.service.status(), BrainStatus::Detached);
    assert!(matches!(
        rig.service.fact(&fact()).await,
        Err(BrainError::NoInstance)
    ));
}

#[tokio::test]
async fn a_failed_probe_leaves_the_brain_unconfirmed() {
    let rig =
        Rig::start(|_| FakeResponse::json(403, &json!({ "error_key": "account_inactive" }))).await;
    crate::support::eventually("the probe asks", || !rig.fake.requests().is_empty()).await;
    assert_eq!(rig.service.status(), BrainStatus::Checking);
    assert!(matches!(
        rig.service.fact(&fact()).await,
        Err(BrainError::Unconfirmed)
    ));
    assert_eq!(rig.engine.tools().len(), 0);
}
