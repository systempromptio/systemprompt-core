use systemprompt_ai::repository::{AiRequestClientEvidenceRepository, AiRequestRepository};
use systemprompt_identifiers::AiRequestId;
use systemprompt_models::wire::origin::{
    ClientAttestation, ClientEvidence, ClientKind, NativeMarker,
};

use super::{completed_record, pool_or_skip, user};

fn evidence() -> ClientEvidence {
    ClientEvidence {
        kind_source: ClientAttestation::HostToken,
        attested_host: Some(ClientKind::Codex),
        declared_client: Some("codex".to_owned()),
        native_marker: Some(NativeMarker::CodexTurnMetadata),
        ua_product: Some("codex_cli_rs".to_owned()),
        ua_version: Some("0.57.0".to_owned()),
        sdk_lang: Some("rust".to_owned()),
        sdk_package_version: Some("0.57.0".to_owned()),
        sdk_runtime: Some("tokio".to_owned()),
        sdk_runtime_version: Some("1.0".to_owned()),
        sdk_os: Some("linux".to_owned()),
        sdk_arch: Some("x86_64".to_owned()),
    }
}

#[tokio::test]
async fn evidence_is_bound_to_a_request_and_a_correction_replaces_every_attribution_field() {
    let pool = pool_or_skip()
        .await
        .expect("AI client evidence fixture database");
    let owner = user();
    systemprompt_test_fixtures::seed_user_row(&pool, &owner, &format!("{owner}@ai.invalid"))
        .await
        .unwrap();
    let request = AiRequestRepository::new(&pool)
        .unwrap()
        .insert(&completed_record(&owner))
        .await
        .unwrap();
    let repository = AiRequestClientEvidenceRepository::new(&pool).unwrap();

    repository.upsert(&request, &evidence()).await.unwrap();
    let correction = ClientEvidence::internal();
    repository.upsert(&request, &correction).await.unwrap();

    assert_eq!(repository.find(&request).await.unwrap(), Some(correction));
    assert!(
        repository
            .find(&AiRequestId::generate())
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn evidence_cannot_be_recorded_for_a_request_outside_the_audit_lifecycle() {
    let pool = pool_or_skip()
        .await
        .expect("AI client evidence fixture database");
    let repository = AiRequestClientEvidenceRepository::new(&pool).unwrap();

    let error = repository
        .upsert(&AiRequestId::generate(), &evidence())
        .await
        .expect_err("the evidence FK must require a durable request row");

    assert!(
        error
            .to_string()
            .contains("ai_request_client_evidence_ai_request_id_fkey")
    );
}
