use chrono::Utc;
use serde_json::json;
use systemprompt_device::client::ConsumerOutcome;
use systemprompt_device::protocol::DeviceCredential;
use systemprompt_identifiers::{InstallationReceiptId, NativeSessionId};
use systemprompt_models::bridge::gateway::SelfEnrollRequest;
use systemprompt_models::feedback::EvaluatorClient;
use systemprompt_models::feedback::receipts::{
    ConsumerReceiptRequest, ReceiptAcknowledgement, SessionBindingRequest,
};
use systemprompt_scripted_instance::{Event, ModelScript, Route};

use crate::support::{instance, signed_in};

const DIGEST: &str = "b3bb6c15ade34221806687bae4d2ff5ccaaf56ab24911bc7859d88dd8c3b9fa4";
const REVISION: &str = "802c1429-27ed-4274-afd9-51e2ed79ea79";

pub(crate) fn enrolment() -> SelfEnrollRequest {
    SelfEnrollRequest {
        fingerprint: "SHA256:ab/cd+ef".to_owned(),
        label: "a test device".to_owned(),
    }
}

fn receipt(installation: &str) -> ConsumerReceiptRequest {
    serde_json::from_value(json!({
        "installation_id": installation,
        "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
        "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
        "revision_id": REVISION,
        "generation": 1,
        "bundle_digest": DIGEST,
        "host": "harness",
        "observed_at": Utc::now(),
        "files": [{
            "revision_id": REVISION,
            "path": "SKILL.md",
            "digest": DIGEST,
            "bytes": 2,
            "executable": false,
            "content_check": "verified",
            "mode_check": "unavailable"
        }],
        "runtime_files": [{
            "path": "SKILL.md",
            "digest": DIGEST,
            "bytes": 2,
            "executable": false,
            "content_check": "verified",
            "mode_check": "unavailable"
        }]
    }))
    .unwrap()
}

#[tokio::test]
async fn a_device_enrols_under_the_user_and_keeps_its_id() {
    let instance = instance(ModelScript::empty()).await;
    let (client, session) = signed_in(&instance).await;
    let first = client
        .enrol_device(&session.jwt, &enrolment())
        .await
        .unwrap();
    assert_eq!(first.consumer_id, instance.user().user_id);
    let second = client
        .enrol_device(&session.jwt, &enrolment())
        .await
        .unwrap();
    assert_eq!(second.device_id, first.device_id);
    assert_ne!(second.credential.expose(), first.credential.expose());

    let record = instance
        .wait_for(Event::Route(Route::DeviceEnrolment))
        .await
        .unwrap();
    assert_eq!(record.body.json().unwrap()["label"], "a test device");
}

#[tokio::test]
async fn a_receipt_is_acknowledged_once_and_recognised_again() {
    let instance = instance(ModelScript::empty()).await;
    let (client, session) = signed_in(&instance).await;
    let device = client
        .enrol_device(&session.jwt, &enrolment())
        .await
        .unwrap()
        .credential;

    let posted = receipt("installation-1");
    let first = client
        .post_receipt(&device, &posted)
        .await
        .into_result()
        .unwrap();
    assert_eq!(first.acknowledgement, ReceiptAcknowledgement::Accepted);
    assert!(first.fully_verified);
    let again = client
        .post_receipt(&device, &posted)
        .await
        .into_result()
        .unwrap();
    assert_eq!(
        again.acknowledgement,
        ReceiptAcknowledgement::IdenticalRetry
    );
    assert_eq!(again.receipt_id, first.receipt_id);
    let other = client
        .post_receipt(&device, &receipt("installation-2"))
        .await
        .into_result()
        .unwrap();
    assert_ne!(other.receipt_id, first.receipt_id);

    let mut empty = receipt("installation-3");
    empty.files.clear();
    assert!(matches!(
        client.post_receipt(&device, &empty).await,
        ConsumerOutcome::Refused(_)
    ));

    let record = instance.wait_for(Event::Receipt).await.unwrap();
    assert_eq!(record.route, Route::Receipt);
    assert!(instance.unscripted_requests().is_empty());
}

#[tokio::test]
async fn a_session_binds_to_an_acknowledged_receipt() {
    let instance = instance(ModelScript::empty()).await;
    let (client, session) = signed_in(&instance).await;
    let device = client
        .enrol_device(&session.jwt, &enrolment())
        .await
        .unwrap()
        .credential;
    let receipt_id = client
        .post_receipt(&device, &receipt("installation-1"))
        .await
        .into_result()
        .unwrap()
        .receipt_id;

    let binding = SessionBindingRequest {
        receipt_id,
        host: EvaluatorClient::Harness,
        session_id: NativeSessionId::new("ses_1"),
    };
    let bound = client
        .post_session_binding(&device, &binding)
        .await
        .into_result()
        .unwrap();
    let again = client
        .post_session_binding(&device, &binding)
        .await
        .into_result()
        .unwrap();
    assert_eq!(again, bound);

    let unknown = SessionBindingRequest {
        receipt_id: InstallationReceiptId::new("r-unknown"),
        ..binding
    };
    assert!(matches!(
        client.post_session_binding(&device, &unknown).await,
        ConsumerOutcome::Refused(_)
    ));
    assert!(
        instance
            .wait_for(Event::Route(Route::SessionBinding))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_credential_enrolment_did_not_issue_is_a_refused_device() {
    let instance = instance(ModelScript::empty()).await;
    let (client, _session) = signed_in(&instance).await;
    let stranger = DeviceCredential::new("sp_device_stranger").unwrap();
    assert!(matches!(
        client
            .post_receipt(&stranger, &receipt("installation-1"))
            .await,
        ConsumerOutcome::DeviceRefused(_)
    ));
}
