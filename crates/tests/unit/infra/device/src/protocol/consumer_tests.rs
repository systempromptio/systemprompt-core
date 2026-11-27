use chrono::{DateTime, Utc};
use systemprompt_device::protocol::names::{session_uuid_string, sha256_hex};
use systemprompt_device::protocol::receipts::{build_receipt, plan_matches};
use systemprompt_device::protocol::wire::DeviceEnrolment;
use systemprompt_identifiers::{ConsumerInstallationId, NativeSessionId};
use systemprompt_models::bridge::consumer::SessionBinding;
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::manifest::ManagedPublicationRef;
use systemprompt_models::feedback::receipts::{
    ConsumerInstallationPlan, ConsumerReceiptResponse, ReadbackStatus, ReceiptAcknowledgement,
    RuntimeFileReadback, SessionBindingRequest,
};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};

const REVISION: &str = "802c1429-27ed-4274-afd9-51e2ed79ea79";

fn publication() -> ManagedPublicationRef {
    serde_json::from_value(serde_json::json!({
        "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
        "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
        "revision_id": REVISION,
        "generation": 1,
        "bundle_digest": "b3bb6c15ade34221806687bae4d2ff5ccaaf56ab24911bc7859d88dd8c3b9fa4"
    }))
    .unwrap()
}

fn plan() -> ConsumerInstallationPlan {
    let skill = b"# Alpha notes\n".to_vec();
    serde_json::from_value(serde_json::json!({
        "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
        "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
        "revision_id": REVISION,
        "generation": 1,
        "bundle_digest": "b3bb6c15ade34221806687bae4d2ff5ccaaf56ab24911bc7859d88dd8c3b9fa4",
        "host": "harness",
        "canonical_files": [{
            "revision_id": REVISION,
            "path": "SKILL.md",
            "digest": sha256_hex(&skill),
            "bytes": skill.len(),
            "executable": false,
            "content_check": "unavailable",
            "mode_check": "unavailable"
        }],
        "runtime_files": [
            { "path": format!(".systemprompt-source/{REVISION}/SKILL.md"), "bytes": skill, "executable": false },
            { "path": "SKILL.md", "bytes": [35, 10], "executable": false }
        ]
    }))
    .unwrap()
}

fn readback(plan: &ConsumerInstallationPlan) -> Vec<RuntimeFileReadback> {
    plan.runtime_files
        .iter()
        .map(|file| RuntimeFileReadback {
            path: file.path.clone(),
            digest: ContentDigest::try_from(sha256_hex(&file.bytes)).unwrap(),
            bytes: file.bytes.len() as u64,
            executable: file.executable,
            content_check: ReadbackStatus::Verified,
            mode_check: ReadbackStatus::Unavailable,
        })
        .collect()
}

#[test]
fn decodes_enrolment_and_refuses_a_bad_credential() {
    let enrolment: DeviceEnrolment = serde_json::from_str(
        r#"{"device_id":"a63f6f68-d4e1-481f-a465-ef8a31ffadbd","consumer_id":"1ecce4e3-bc87-441d-b374-ea29bab88da8","credential":"sp_device_abc123"}"#,
    )
    .unwrap();
    assert_eq!(
        enrolment.device_id.as_str(),
        "a63f6f68-d4e1-481f-a465-ef8a31ffadbd"
    );
    assert!(
        serde_json::from_str::<DeviceEnrolment>(
            r#"{"device_id":"d","consumer_id":"u","credential":"not-a-device-credential"}"#
        )
        .is_err()
    );
}

#[test]
fn a_plan_must_be_the_one_the_manifest_named() {
    let plan = plan();
    assert!(plan_matches(&plan, &publication(), HostKind::Harness));
    assert!(!plan_matches(&plan, &publication(), HostKind::ClaudeCode));
    let mut other = publication();
    other.generation = 2;
    assert!(!plan_matches(&plan, &other, HostKind::Harness));
}

#[test]
fn a_byte_out_of_range_fails_the_plan() {
    let mut value = serde_json::to_value(plan()).unwrap();
    value["runtime_files"][1]["bytes"] = serde_json::json!([35, 256]);
    assert!(serde_json::from_value::<ConsumerInstallationPlan>(value).is_err());
}

#[test]
fn builds_a_sorted_fully_verified_receipt() {
    let plan = plan();
    let at = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    let receipt = build_receipt(
        &plan,
        &readback(&plan),
        ConsumerInstallationId::new("inst-1"),
        at,
    )
    .unwrap();
    assert_eq!(
        receipt.runtime_files[0].path,
        format!(".systemprompt-source/{REVISION}/SKILL.md")
    );
    assert_eq!(receipt.runtime_files[1].path, "SKILL.md");
    assert_eq!(receipt.files[0].content_check, ReadbackStatus::Verified);
    assert!(receipt.fully_verified());
    assert_eq!(receipt.host, EvaluatorClient::Harness);
}

#[test]
fn a_readback_that_differs_names_the_path() {
    let plan = plan();
    let mut read = readback(&plan);
    read[1].bytes = 99;
    let at = DateTime::<Utc>::from_timestamp(1_800_000_000, 0).unwrap();
    let error = build_receipt(&plan, &read, ConsumerInstallationId::new("inst-1"), at).unwrap_err();
    assert_eq!(error.path, "SKILL.md");
    let missing =
        build_receipt(&plan, &read[..0], ConsumerInstallationId::new("inst-1"), at).unwrap_err();
    assert_eq!(
        missing.path,
        format!(".systemprompt-source/{REVISION}/SKILL.md")
    );
}

#[test]
fn decodes_acknowledgements_and_bindings() {
    let ack: ConsumerReceiptResponse = serde_json::from_str(
        r#"{"receipt_id":"d94cc1d5-e695-4a34-9eb6-ae4cd8baa9e4","acknowledgement":"identical_retry","acknowledged_at":"2026-11-01T19:22:02.397276Z","fully_verified":true}"#,
    )
    .unwrap();
    assert_eq!(ack.acknowledgement, ReceiptAcknowledgement::IdenticalRetry);
    let unknown = serde_json::from_str::<ReceiptAcknowledgement>("\"superseded\"");
    assert!(unknown.is_ok(), "an unknown acknowledgement: {unknown:?}");
    let binding: SessionBinding =
        serde_json::from_str(r#"{"id":"b-1","bound_at":"2026-11-01T19:22:02Z"}"#).unwrap();
    assert_eq!(binding.id.as_str(), "b-1");
    let body = SessionBindingRequest {
        receipt_id: ack.receipt_id,
        host: EvaluatorClient::Harness,
        session_id: NativeSessionId::new(session_uuid_string("ses_1")),
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["host"], "harness");
}
