//! The instance's refusals map to failures the shell and the agent can act on.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use systemprompt_identifiers::BrainFactId;

use serde_json::json;
use systemprompt_device::client::ClientError;
use systemprompt_harness::members::brain::{BrainError, FactQuery};

use crate::members::brain::support::{CountingJwt, FACT, FakeInstance, FakeResponse, error_json};

fn fact() -> BrainFactId {
    BrainFactId::try_new(FACT).unwrap()
}

async fn refusal(status: u16, key: Option<&str>) -> BrainError {
    let body = key.map_or_else(
        || json!({ "code": "not_found", "message": "Not Found" }),
        error_json,
    );
    let fake = FakeInstance::start(move |_| FakeResponse::json(status, &body)).await;
    fake.brain().fact(&fact()).await.unwrap_err()
}

#[tokio::test]
async fn a_brain_the_instance_did_not_enable_reads_as_disabled() {
    assert!(matches!(
        refusal(404, Some("brain_disabled")).await,
        BrainError::Disabled
    ));
    assert!(matches!(refusal(404, None).await, BrainError::Disabled));
}

#[tokio::test]
async fn a_hidden_or_missing_fact_reads_as_not_found() {
    let error = refusal(404, Some("fact_not_found")).await;
    assert!(matches!(error, BrainError::NotFound { .. }));
    assert_eq!(error.error_key(), Some("fact_not_found"));
}

#[tokio::test]
async fn refusals_keep_their_kind_and_key() {
    assert!(matches!(
        refusal(403, Some("brain_forbidden")).await,
        BrainError::Forbidden { error_key: Some(ref key), .. } if key == "brain_forbidden"
    ));
    assert!(matches!(
        refusal(409, Some("fact_tombstoned")).await,
        BrainError::Tombstoned { .. }
    ));
    assert!(matches!(
        refusal(409, Some("conflict")).await,
        BrainError::Conflict { .. }
    ));
    assert!(matches!(
        refusal(422, Some("unknown_kind")).await,
        BrainError::Invalid { error_key: Some(ref key), .. } if key == "unknown_kind"
    ));
}

#[tokio::test]
async fn an_unavailable_brain_is_transient() {
    let error = refusal(503, Some("brain_unavailable")).await;
    assert!(error.is_transient());
    assert!(matches!(
        error,
        BrainError::Client(ClientError::Unreachable { .. })
    ));
}

#[tokio::test]
async fn a_refused_token_is_minted_again_once_then_reported() {
    let fake = FakeInstance::start(|_| FakeResponse::json(401, &json!({}))).await;
    let credentials = Arc::new(CountingJwt::default());
    let error = fake
        .brain_with(Arc::clone(&credentials))
        .facts(&FactQuery::default())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        BrainError::Client(ClientError::Unauthorized { .. })
    ));
    assert_eq!(fake.requests().len(), 2);
    assert_eq!(credentials.minted.load(Ordering::SeqCst), 2);
    assert_eq!(credentials.rejected.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_answer_that_does_not_decode_is_a_protocol_error() {
    let fake = FakeInstance::start(|_| FakeResponse::json(200, &json!({ "facts": "none" }))).await;
    let error = fake.brain().facts(&FactQuery::default()).await.unwrap_err();
    assert!(matches!(
        error,
        BrainError::Client(ClientError::Protocol {
            status: Some(200),
            ..
        })
    ));
}
