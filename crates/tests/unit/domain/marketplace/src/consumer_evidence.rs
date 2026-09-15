use crate::consumer_fixture::fixture;
use systemprompt_identifiers::{DeviceCertId, NativeSessionId};
use systemprompt_models::feedback::receipts::{ReadbackStatus, ReceiptAcknowledgement};
use systemprompt_models::feedback::{ContentDigest, EvaluatorClient};

#[tokio::test]
async fn consumer_identity_is_derived_from_enrolled_device_not_publisher_or_body() {
    let f = fixture().await;
    assert_ne!(f.owner, f.consumer);
    let identity = f
        .repo
        .authenticate_consumer_device(&f.credential.credential)
        .await
        .unwrap();
    assert_eq!(identity.consumer_id, f.consumer);
    assert_eq!(identity.device_id.as_str(), f.cert.as_str());
    assert!(
        f.repo
            .authenticate_consumer_device(f.cert.as_str())
            .await
            .is_err()
    );
    assert!(
        f.repo
            .authenticate_consumer_device("per-user-bridge-secret")
            .await
            .is_err()
    );
    assert!(
        f.repo
            .issue_consumer_credential(&DeviceCertId::generate())
            .await
            .is_err()
    );
    let receipt = f
        .repo
        .record_consumer_receipt(&f.credential.credential, &f.request)
        .await
        .unwrap();
    let row = sqlx::query!(
        "SELECT owner_id,consumer_id,device_id FROM managed_installation_receipts WHERE id=$1",
        receipt.receipt_id.as_str()
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(row.owner_id, f.owner.as_str());
    assert_eq!(row.consumer_id.as_deref(), Some(f.consumer.as_str()));
    assert_eq!(row.device_id.as_deref(), Some(f.cert.as_str()));
    let mut forged = serde_json::to_value(&f.request).unwrap();
    forged["consumer_id"] = serde_json::json!(f.owner);
    assert!(
        serde_json::from_value::<systemprompt_models::feedback::receipts::ConsumerReceiptRequest>(
            forged
        )
        .is_err()
    );
}

#[tokio::test]
async fn identical_retries_acknowledge_but_conflicting_observations_are_rejected() {
    let f = fixture().await;
    let first = f
        .repo
        .record_consumer_receipt(&f.credential.credential, &f.request)
        .await
        .unwrap();
    let mut reordered = f.request.clone();
    reordered.files.reverse();
    let retry = f
        .repo
        .record_consumer_receipt(&f.credential.credential, &reordered)
        .await
        .unwrap();
    assert_eq!(first.receipt_id, retry.receipt_id);
    assert_eq!(
        retry.acknowledgement,
        ReceiptAcknowledgement::IdenticalRetry
    );
    reordered.files[0].mode_check = ReadbackStatus::Unavailable;
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &reordered)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn tampered_content_modes_partial_installs_and_wrong_generations_fail() {
    let f = fixture().await;
    let mut tampered = f.request.clone();
    tampered.files[0].digest = ContentDigest::of(b"tampered");
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &tampered)
            .await
            .is_err()
    );
    tampered = f.request.clone();
    tampered.files[0].executable = !tampered.files[0].executable;
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &tampered)
            .await
            .is_err()
    );
    tampered = f.request.clone();
    tampered.files.pop();
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &tampered)
            .await
            .is_err()
    );
    tampered = f.request.clone();
    tampered.generation += 1;
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &tampered)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn unavailable_platform_checks_are_retained_without_verified_session_binding() {
    let mut f = fixture().await;
    f.request.files[0].mode_check = ReadbackStatus::Unavailable;
    let binding = f.receipt_binding().await;
    let receipt = f
        .repo
        .consumer_receipt_status(&f.credential.credential, &binding.receipt_id)
        .await
        .unwrap();
    assert!(!receipt.fully_verified);
    assert!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &binding)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn grant_changes_credential_rotation_and_certificate_revocation_are_enforced() {
    let f = fixture().await;
    f.grant(false).await;
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &f.request)
            .await
            .is_err()
    );
    f.grant(true).await;
    let rotated = f.repo.issue_consumer_credential(&f.cert).await.unwrap();
    assert!(
        f.repo
            .authenticate_consumer_device(&f.credential.credential)
            .await
            .is_err()
    );
    assert!(
        f.repo
            .record_consumer_receipt(&rotated.credential, &f.request)
            .await
            .is_ok()
    );
    sqlx::query!(
        "UPDATE user_device_certs SET revoked_at=now() WHERE id=$1",
        f.cert.as_str()
    )
    .execute(&f.pool)
    .await
    .unwrap();
    assert!(
        f.repo
            .authenticate_consumer_device(&rotated.credential)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn late_receipt_and_session_binding_correct_unknown_without_duplicate_usage() {
    let f = fixture().await;
    let input = f.invocation();
    let unknown = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &input)
        .await
        .unwrap();
    assert!(unknown.receipt_id.is_none());
    let binding = f.receipt_binding().await;
    let before_session = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &input)
        .await
        .unwrap();
    assert!(before_session.receipt_id.is_none());
    f.repo
        .bind_consumer_session(&f.credential.credential, &binding)
        .await
        .unwrap();
    let corrected = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &input)
        .await
        .unwrap();
    assert_eq!(corrected.receipt_id, Some(binding.receipt_id));
    assert_eq!(corrected.version, 2);
    let count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM managed_consumer_invocation_evidence WHERE consumer_id=$1",
        f.consumer.as_str()
    )
    .fetch_one(&f.pool)
    .await
    .unwrap();
    assert_eq!(count, Some(1));
    let history = sqlx::query_scalar!("SELECT COUNT(*) FROM managed_consumer_attribution_history h JOIN managed_consumer_invocation_evidence e ON e.id=h.evidence_id WHERE e.consumer_id=$1", f.consumer.as_str()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(history, Some(2));
    let mut conflicting = input;
    conflicting.evidence = serde_json::json!({"forged":true});
    assert!(
        f.repo
            .record_consumer_invocation(&f.credential.credential, &conflicting)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn foreign_device_host_and_unbound_session_cannot_attribute_receipt() {
    let f = fixture().await;
    let binding = f.receipt_binding().await;
    let other = fixture().await;
    assert!(
        f.repo
            .bind_consumer_session(&other.credential.credential, &binding)
            .await
            .is_err()
    );
    let mut wrong_host = binding.clone();
    wrong_host.host = EvaluatorClient::Hermes;
    assert!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &wrong_host)
            .await
            .is_err()
    );
    f.repo
        .bind_consumer_session(&f.credential.credential, &binding)
        .await
        .unwrap();
    let mut input = f.invocation();
    input.session_id = NativeSessionId::new("unbound-session");
    assert!(
        f.repo
            .record_consumer_invocation(&f.credential.credential, &input)
            .await
            .unwrap()
            .receipt_id
            .is_none()
    );
}

#[tokio::test]
async fn concurrent_binding_and_ingestion_converge_and_retries_do_not_change_history() {
    let f = fixture().await;
    let binding = f.receipt_binding().await;
    let input = f.invocation();
    let (bound, ingested) = tokio::join!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &binding),
        f.repo
            .record_consumer_invocation(&f.credential.credential, &input),
    );
    bound.unwrap();
    ingested.unwrap();
    let corrected = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &input)
        .await
        .unwrap();
    assert_eq!(corrected.receipt_id, Some(binding.receipt_id.clone()));
    f.repo
        .bind_consumer_session(&f.credential.credential, &binding)
        .await
        .unwrap();
    assert_eq!(
        f.repo
            .record_consumer_invocation(&f.credential.credential, &input)
            .await
            .unwrap()
            .version,
        corrected.version
    );
}

#[tokio::test]
async fn catalog_cannot_restore_explicit_revocation_and_revoked_credentials_fail() {
    let f = fixture().await;
    f.grant(false).await;
    f.repo
        .retain_consumer_catalog_grant(&f.owner, &f.request.resource_id, &f.consumer)
        .await
        .unwrap();
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &f.request)
            .await
            .is_err()
    );
    f.repo.revoke_consumer_credential(&f.cert).await.unwrap();
    assert!(
        f.repo
            .authenticate_consumer_device(&f.credential.credential)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn historical_receipts_keep_unknown_consumer_and_device_without_session_binding() {
    use systemprompt_marketplace::managed::{
        AssetDigest, ClientEvidence, InstallationReceiptRequest, InstalledFile,
    };
    let f = fixture().await;
    let delivery = f
        .repo
        .claim_distribution(&f.owner, "historical-distribution")
        .await
        .unwrap()
        .unwrap();
    f.repo
        .complete_distribution(&f.owner, &delivery, true, None)
        .await
        .unwrap();
    let historical = InstallationReceiptRequest {
        installation_id: systemprompt_identifiers::ConsumerInstallationId::new(
            "historical-install",
        ),
        publication_id: f.request.publication_id.clone(),
        resource_id: f.request.resource_id.clone(),
        generation: f.request.generation,
        bundle_digest: AssetDigest::try_from(f.request.bundle_digest.as_str().to_owned()).unwrap(),
        files: f
            .request
            .files
            .iter()
            .map(|file| InstalledFile {
                revision_id: file.revision_id.clone(),
                path: file.path.clone(),
                digest: AssetDigest::try_from(file.digest.as_str().to_owned()).unwrap(),
                bytes: file.bytes,
                executable: file.executable,
            })
            .collect(),
        client_evidence: ClientEvidence {
            session_id: systemprompt_identifiers::SessionId::new("historical-session"),
            owner_id: f.owner.clone(),
            recorded: std::collections::BTreeMap::new(),
        },
    };
    let receipt = f
        .repo
        .record_installation(&f.owner, &historical)
        .await
        .unwrap();
    let row = sqlx::query!("SELECT consumer_id,device_id,fully_verified FROM managed_installation_receipts WHERE id=$1", receipt.id.as_str()).fetch_one(&f.pool).await.unwrap();
    assert!(row.consumer_id.is_none());
    assert!(row.device_id.is_none());
    assert!(row.fully_verified.is_none());
    let binding = systemprompt_models::feedback::receipts::SessionBindingRequest {
        receipt_id: receipt.id,
        host: f.request.host,
        session_id: NativeSessionId::new("historical-session"),
    };
    assert!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &binding)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rollback_requires_new_session_and_preserves_original_attribution_after_history_loss() {
    use systemprompt_marketplace::managed::{PublicationAction, PublicationRequest};
    let mut f = fixture().await;
    let original = f.receipt_binding().await;
    f.repo
        .bind_consumer_session(&f.credential.credential, &original)
        .await
        .unwrap();
    let old_input = f.invocation();
    let old_attribution = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &old_input)
        .await
        .unwrap();
    assert_eq!(
        old_attribution.receipt_id,
        Some(original.receipt_id.clone())
    );
    let rollback = f
        .repo
        .review_and_publish(
            &f.owner,
            &f.owner,
            &PublicationRequest {
                resource_id: f.request.resource_id.clone(),
                revision_id: Some(f.request.revision_id.clone()),
                action: PublicationAction::Rollback,
                expected_generation: 1,
                operation_key: "rollback".to_owned(),
                comparison_evidence: systemprompt_marketplace::managed::ComparisonEvidence::default(
                ),
                limitations: String::new(),
            },
        )
        .await
        .unwrap();
    f.request.publication_id = rollback.publication_id;
    f.request.generation = rollback.generation;
    let mut rebound = f.receipt_binding().await;
    // A client that has compacted its completed local history may retry this
    // original native session against the newly installed publication.
    assert!(matches!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &rebound)
            .await,
        Err(systemprompt_marketplace::managed::ManagedError::Conflict(_))
    ));
    assert_eq!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &original)
            .await
            .unwrap()
            .id,
        f.repo
            .bind_consumer_session(&f.credential.credential, &original)
            .await
            .unwrap()
            .id
    );
    rebound.session_id = NativeSessionId::new("new-session-after-rollback");
    f.repo
        .bind_consumer_session(&f.credential.credential, &rebound)
        .await
        .unwrap();
    assert_ne!(original.receipt_id, rebound.receipt_id);
    let mut new_input = f.invocation();
    new_input.invocation_id =
        systemprompt_identifiers::ResourceInvocationId::new("rollback-invocation");
    new_input.session_id = rebound.session_id.clone();
    assert_eq!(
        f.repo
            .record_consumer_invocation(&f.credential.credential, &new_input)
            .await
            .unwrap()
            .receipt_id,
        Some(rebound.receipt_id)
    );
    assert_eq!(
        f.repo
            .record_consumer_invocation(&f.credential.credential, &old_input)
            .await
            .unwrap()
            .receipt_id,
        Some(original.receipt_id)
    );
}

#[tokio::test]
async fn active_script_and_entrypoint_checks_cannot_be_replaced_by_source_cache_proof() {
    let f = fixture().await;
    let mut cache_only = f.request.clone();
    cache_only
        .runtime_files
        .retain(|file| file.path.starts_with(".systemprompt-source/"));
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &cache_only)
            .await
            .is_err()
    );
    let mut tampered = f.request.clone();
    tampered
        .runtime_files
        .iter_mut()
        .find(|file| file.path == "run.sh")
        .unwrap()
        .digest = ContentDigest::of(b"tampered runtime script");
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &tampered)
            .await
            .is_err()
    );
    let mut tampered = f.request.clone();
    tampered
        .runtime_files
        .iter_mut()
        .find(|file| file.path == "SKILL.md")
        .unwrap()
        .digest = ContentDigest::of(b"tampered active instructions");
    assert!(
        f.repo
            .record_consumer_receipt(&f.credential.credential, &tampered)
            .await
            .is_err()
    );
    let mut unavailable = f.request.clone();
    unavailable.runtime_files.clear();
    let receipt = f
        .repo
        .record_consumer_receipt(&f.credential.credential, &unavailable)
        .await
        .unwrap();
    assert!(!receipt.fully_verified);
}

#[tokio::test]
async fn pending_certificate_revocation_fences_credential_issuance() {
    let f = fixture().await;
    let mut tx = f.pool.begin().await.unwrap();
    sqlx::query("UPDATE user_device_certs SET revoked_at=now() WHERE id=$1")
        .bind(f.cert.as_str())
        .execute(&mut *tx)
        .await
        .unwrap();
    let repo = f.repo.clone();
    let cert = f.cert.clone();
    let mut issue = tokio::spawn(async move { repo.issue_consumer_credential(&cert).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut issue)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), issue)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert!(
        f.repo
            .authenticate_consumer_device(&f.credential.credential)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn users_device_interface_retains_shared_lock_until_caller_commit() {
    let f = fixture().await;
    let mut tx = f.pool.begin().await.unwrap();
    let identity: String =
        sqlx::query_scalar("SELECT consumer_id FROM public.active_device_identity($1)")
            .bind(f.cert.as_str())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(identity, f.consumer.as_str());
    let pool = f.pool.clone();
    let cert = f.cert.clone();
    let mut revoke = tokio::spawn(async move {
        sqlx::query("UPDATE user_device_certs SET revoked_at=now() WHERE id=$1")
            .bind(cert.as_str())
            .execute(&pool)
            .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut revoke)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), revoke)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .rows_affected(),
        1
    );
    let devices: i64 =
        sqlx::query_scalar("SELECT count(*) FROM public.active_devices_for_consumer($1)")
            .bind(f.consumer.as_str())
            .fetch_one(&f.pool)
            .await
            .unwrap();
    assert_eq!(devices, 0);
}


#[tokio::test]
async fn same_native_session_can_bind_independent_resources() {
    let first = fixture().await;
    let second = fixture().await;
    let binding = first.receipt_binding().await;
    first
        .repo
        .bind_consumer_session(&first.credential.credential, &binding)
        .await
        .unwrap();
    second
        .repo
        .set_consumer_grant(
            &second.owner,
            &second.request.resource_id,
            &first.consumer,
            true,
        )
        .await
        .unwrap();
    let receipt = second
        .repo
        .record_consumer_receipt(&first.credential.credential, &second.request)
        .await
        .unwrap();
    let other = systemprompt_models::feedback::receipts::SessionBindingRequest {
        receipt_id: receipt.receipt_id,
        host: binding.host,
        session_id: binding.session_id,
    };
    let bound = first
        .repo
        .bind_consumer_session(&first.credential.credential, &other)
        .await
        .unwrap();
    assert_eq!(
        bound.id,
        first
            .repo
            .bind_consumer_session(&first.credential.credential, &other)
            .await
            .unwrap()
            .id
    );
}


#[tokio::test]
async fn historical_multiple_receipts_ack_identical_retry_without_inventing_attribution() {
    use systemprompt_identifiers::{ConsumerInstallationId, InstallationSessionBindingId};
    use systemprompt_marketplace::managed::ManagedError;
    use systemprompt_marketplace::managed::consumer::host_key;
    let mut f = fixture().await;
    let original = f.receipt_binding().await;
    let original_bound = f
        .repo
        .bind_consumer_session(&f.credential.credential, &original)
        .await
        .unwrap();
    f.request.installation_id = ConsumerInstallationId::generate();
    let historical = f.receipt_binding().await;
    let historical_id = InstallationSessionBindingId::generate();
    // Seed exactly the retained state allowed before first-binding admission.
    sqlx::query("INSERT INTO managed_consumer_session_bindings(id,receipt_id,consumer_id,device_id,host,native_session_id) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(historical_id.as_str()).bind(historical.receipt_id.as_str())
        .bind(f.consumer.as_str()).bind(f.cert.as_str()).bind(host_key(f.request.host))
        .bind(original.session_id.as_str()).execute(&f.pool).await.unwrap();
    let mut evidence = f.invocation();
    evidence.installation_id = None;
    let before = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &evidence)
        .await
        .unwrap();
    assert!(before.receipt_id.is_none());
    let retry = f
        .repo
        .bind_consumer_session(&f.credential.credential, &original)
        .await
        .unwrap();
    assert_eq!(retry.id, original_bound.id);
    assert_eq!(retry.bound_at, original_bound.bound_at);
    assert_eq!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &historical)
            .await
            .unwrap()
            .id,
        historical_id
    );
    let after = f
        .repo
        .record_consumer_invocation(&f.credential.credential, &evidence)
        .await
        .unwrap();
    assert!(after.receipt_id.is_none());
    assert_eq!(after.version, before.version);
    f.request.installation_id = ConsumerInstallationId::generate();
    let third = f.receipt_binding().await;
    assert!(matches!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &third)
            .await,
        Err(ManagedError::Conflict(_))
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM managed_consumer_session_bindings WHERE consumer_id=$1 AND device_id=$2 AND host=$3 AND native_session_id=$4")
        .bind(f.consumer.as_str()).bind(f.cert.as_str()).bind(host_key(f.request.host))
        .bind(original.session_id.as_str()).fetch_one(&f.pool).await.unwrap();
    assert_eq!(count, 2);
    f.grant(false).await;
    assert!(
        f.repo
            .bind_consumer_session(&f.credential.credential, &original)
            .await
            .is_err()
    );
}
