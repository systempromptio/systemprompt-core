//! Fencing, crash recovery, credential secrecy and immutable capture retries.
use crate::consumer_fixture::fixture;
use systemprompt_identifiers::TaskId;
use systemprompt_marketplace::managed::SourceSpec;
use systemprompt_marketplace::managed::operations::ApiOperationClaim;
fn acquired(
    claim: ApiOperationClaim,
) -> systemprompt_marketplace::managed::operations::ApiOperation {
    match claim {
        ApiOperationClaim::Acquired(operation) => operation,
        other => panic!("expected acquisition: {other:?}"),
    }
}
#[tokio::test]
async fn expired_claim_cannot_borrow_successors_fence_or_replace_checkpoint() {
    let f = fixture().await;
    let key = TaskId::generate();
    let first = acquired(
        f.repo
            .begin_api_operation(&f.owner, &key, "inventory_refresh", &"original")
            .await
            .unwrap(),
    );
    assert!(matches!(
        f.repo
            .begin_api_operation(&f.owner, &key, "inventory_refresh", &"original")
            .await
            .unwrap(),
        ApiOperationClaim::Retained(_)
    ));
    f.repo
        .checkpoint_api_input(&f.owner, &first, &vec!["frozen".to_owned()])
        .await
        .unwrap();
    assert!(
        f.repo
            .begin_api_operation(&f.owner, &key, "inventory_refresh", &"conflict")
            .await
            .is_err()
    );
    sqlx::query("UPDATE managed_api_operations SET lease_until=clock_timestamp()-interval '1 second' WHERE owner_id=$1 AND id=$2").bind(f.owner.as_str()).bind(key.as_str()).execute(&f.pool).await.unwrap();
    let successor = acquired(
        f.repo
            .begin_api_operation(&f.owner, &key, "inventory_refresh", &"original")
            .await
            .unwrap(),
    );
    assert!(successor.fence > first.fence);
    assert!(
        f.repo
            .finish_api_operation(&f.owner, &first, &"stale")
            .await
            .is_err()
    );
    assert!(
        f.repo
            .checkpoint_api_input(&f.owner, &first, &vec!["changed".to_owned()])
            .await
            .is_err()
    );
    let checkpoint: Vec<String> = f
        .repo
        .checkpoint_api_input(&f.owner, &successor, &vec!["changed".to_owned()])
        .await
        .unwrap();
    assert_eq!(checkpoint, vec!["frozen"]);
    f.repo
        .finish_api_operation(&f.owner, &successor, &"done")
        .await
        .unwrap();
    let retained = f.repo.api_operation(&f.owner, &key).await.unwrap();
    assert_eq!(retained.state, "completed");
    assert_eq!(retained.result, Some(serde_json::json!("done")));
    assert!(f.repo.api_operation(&f.consumer, &key).await.is_err());
}
#[tokio::test]
async fn credential_retry_preserves_token_digest_without_retaining_plaintext() {
    let f = fixture().await;
    let key = TaskId::generate();
    let operation = acquired(
        f.repo
            .begin_api_operation(&f.owner, &key, "credential_issue", &f.cert)
            .await
            .unwrap(),
    );
    let (_, token) = f
        .repo
        .issue_api_consumer_credential(&f.owner, &operation, &f.cert)
        .await
        .unwrap();
    let token = token.unwrap();
    assert!(f.repo.authenticate_consumer_device(&token).await.is_ok());
    let status = f.repo.api_operation(&f.owner, &key).await.unwrap();
    assert!(!serde_json::to_string(&status).unwrap().contains(&token));
    let (again, retry_token) = f
        .repo
        .issue_api_consumer_credential(&f.owner, &operation, &f.cert)
        .await
        .unwrap();
    assert!(retry_token.is_none());
    assert!(!again.token_retrievable);
    assert!(f.repo.authenticate_consumer_device(&token).await.is_ok());
    let next = acquired(
        f.repo
            .begin_api_operation(&f.owner, &TaskId::generate(), "credential_issue", &f.cert)
            .await
            .unwrap(),
    );
    let (_, new) = f
        .repo
        .issue_api_consumer_credential(&f.owner, &next, &f.cert)
        .await
        .unwrap();
    assert!(
        f.repo
            .authenticate_consumer_device(&new.unwrap())
            .await
            .is_ok()
    );
    assert!(f.repo.authenticate_consumer_device(&token).await.is_err());
}
#[tokio::test]
async fn capture_restart_reuses_snapshot_and_revisions_after_source_changes() {
    let f = fixture().await;
    let root = tempfile::tempdir().unwrap();
    let skill = root.path().join("skills/restart");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("config.yaml"),
        "id: restart\nname: Restart\ndescription: Recovery fixture\n",
    )
    .unwrap();
    std::fs::write(skill.join("index.md"), "# Retained first contents").unwrap();
    let source = f
        .repo
        .register_source(
            &f.owner,
            "api-capture-fixture",
            &SourceSpec::LocalTree {
                root: root.path().to_string_lossy().into_owned(),
            },
        )
        .await
        .unwrap();
    let key = TaskId::generate();
    let first = acquired(
        f.repo
            .begin_api_operation(&f.owner, &key, "source_capture", &source)
            .await
            .unwrap(),
    );
    let captured =
        systemprompt_marketplace::managed::capture_skills(root.path(), &["restart".to_owned()])
            .unwrap();
    let captured = f
        .repo
        .checkpoint_api_input(&f.owner, &first, &captured)
        .await
        .unwrap();
    let initial = f
        .repo
        .import_api_capture(&f.owner, &first, &source, &captured)
        .await
        .unwrap();
    std::fs::write(skill.join("index.md"), "# Changed after response loss").unwrap();
    sqlx::query("UPDATE managed_api_operations SET lease_until=clock_timestamp()-interval '1 second' WHERE owner_id=$1 AND id=$2").bind(f.owner.as_str()).bind(key.as_str()).execute(&f.pool).await.unwrap();
    let retry = acquired(
        f.repo
            .begin_api_operation(&f.owner, &key, "source_capture", &source)
            .await
            .unwrap(),
    );
    let retained = f
        .repo
        .api_input::<systemprompt_marketplace::managed::CapturedSkills>(&f.owner, &retry)
        .await
        .unwrap()
        .unwrap();
    let recovered = f
        .repo
        .import_api_capture(&f.owner, &retry, &source, &retained)
        .await
        .unwrap();
    assert_eq!(recovered.snapshot_id, initial.snapshot_id);
    assert_eq!(recovered.revisions, initial.revisions);
    f.repo
        .finish_api_operation(&f.owner, &retry, &recovered)
        .await
        .unwrap();
}

#[tokio::test]
async fn inventory_completion_and_generation_are_atomic_across_response_loss() {
    let f = fixture().await;
    let key = TaskId::generate();
    let claim = acquired(
        f.repo
            .begin_api_operation(&f.owner, &key, "inventory_refresh", &())
            .await
            .unwrap(),
    );
    let first = f
        .repo
        .reconcile_inventory_operation(&f.owner, &[], Some(&claim))
        .await
        .unwrap();
    assert_eq!(
        f.repo.api_operation(&f.owner, &key).await.unwrap().state,
        "completed"
    );
    let repeated = f
        .repo
        .reconcile_inventory_operation(&f.owner, &[], Some(&claim))
        .await
        .unwrap();
    assert_eq!(repeated.generation, first.generation);
    assert!(matches!(
        f.repo
            .begin_api_operation(&f.owner, &key, "inventory_refresh", &())
            .await
            .unwrap(),
        ApiOperationClaim::Retained(_)
    ));
}
