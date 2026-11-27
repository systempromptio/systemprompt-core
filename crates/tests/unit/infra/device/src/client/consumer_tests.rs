use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use serde_json::json;
use systemprompt_device::client::{ClientError, ConsumerOutcome};
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::manifest::ManagedPublicationRef;

use crate::client::support::{FakeInstance, FakeResponse};

fn publication() -> ManagedPublicationRef {
    serde_json::from_value(json!({
        "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
        "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
        "revision_id": "802c1429-27ed-4274-afd9-51e2ed79ea79",
        "generation": 1,
        "bundle_digest": "b3bb6c15ade34221806687bae4d2ff5ccaaf56ab24911bc7859d88dd8c3b9fa4"
    }))
    .unwrap()
}

#[tokio::test]
async fn consumer_answers_are_classified() {
    let generation = Arc::new(AtomicUsize::new(1));
    let served = Arc::clone(&generation);
    let fake = FakeInstance::start(move |request| match request.path() {
        "/api/v1/consumer/resources/1d1ee2f7-5968-48c6-abc6-2c993721f681/publications/750981ce-807d-4c97-8607-0d510dd51a63/bundle" => {
            FakeResponse::json(
                200,
                &json!({
                    "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
                    "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
                    "revision_id": "802c1429-27ed-4274-afd9-51e2ed79ea79",
                    "generation": served.load(Ordering::SeqCst),
                    "bundle_digest": "b3bb6c15ade34221806687bae4d2ff5ccaaf56ab24911bc7859d88dd8c3b9fa4",
                    "host": "harness",
                    "canonical_files": [],
                    "runtime_files": [{ "path": "SKILL.md", "bytes": [35, 10], "executable": false }]
                }),
            )
        },
        ApiPaths::CONSUMER_RECEIPTS => FakeResponse::json(403, &json!({ "title": "Forbidden", "status": 403, "detail": "Consumer evidence is unavailable to this identity" })),
        ApiPaths::CONSUMER_SESSION_BINDINGS => FakeResponse::json(401, &json!({ "detail": "Valid enrolled-device authentication is required" })),
        _ => FakeResponse::status(404),
    })
    .await;
    let client = fake.client();
    let device = DeviceCredential::new("sp_device_abc").unwrap();

    let plan = client
        .installation_plan(&device, &publication())
        .await
        .into_result()
        .unwrap();
    assert_eq!(plan.runtime_files[0].bytes, b"#\n");
    let sent = fake.last();
    assert_eq!(sent.query(), "host=harness");
    assert_eq!(sent.header("authorization"), Some("Bearer sp_device_abc"));

    generation.store(2, Ordering::SeqCst);
    assert!(matches!(
        client.installation_plan(&device, &publication()).await,
        ConsumerOutcome::RetryLater(_)
    ));

    let binding = systemprompt_models::feedback::receipts::SessionBindingRequest {
        receipt_id: systemprompt_identifiers::InstallationReceiptId::new("r1"),
        host: systemprompt_models::feedback::EvaluatorClient::Harness,
        session_id: systemprompt_identifiers::NativeSessionId::new("ses_1"),
    };
    assert!(matches!(
        client.post_session_binding(&device, &binding).await,
        ConsumerOutcome::DeviceRefused(_)
    ));

    let refused: ConsumerOutcome<()> = ConsumerOutcome::classify(Err(ClientError::Protocol {
        endpoint: "POST /api/v1/consumer/receipts",
        status: Some(409),
        what: "conflict".to_owned(),
        detail: None,
    }));
    assert!(matches!(refused, ConsumerOutcome::Refused(_)));
    let limited: ConsumerOutcome<()> = ConsumerOutcome::classify(Err(ClientError::RateLimited {
        endpoint: "POST /api/v1/consumer/receipts",
        retry_after: Some(Duration::from_secs(9)),
    }));
    assert_eq!(limited.retry_after(), Some(Duration::from_secs(9)));
}

#[test]
fn outcomes_hand_back_the_result() {
    let acknowledged: ConsumerOutcome<u8> = ConsumerOutcome::classify(Ok(7));
    assert_eq!(acknowledged.retry_after(), None);
    assert_eq!(acknowledged.into_result().unwrap(), 7);
    let device: ConsumerOutcome<u8> = ConsumerOutcome::classify(Err(ClientError::Unauthorized {
        endpoint: "POST /api/v1/consumer/receipts",
        error_key: None,
    }));
    assert!(matches!(device, ConsumerOutcome::DeviceRefused(_)));
    assert!(device.into_result().unwrap_err().is_unauthorized());
}
