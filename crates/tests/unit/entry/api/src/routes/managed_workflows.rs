//! Managed administration persists source state and keeps missing resources
//! distinguishable from successful reads.

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use std::collections::BTreeMap;
use std::sync::Arc;
use systemprompt_analytics::snapshots::{FeedbackSnapshot, LatencyHistogram, SnapshotMetrics};
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt_marketplace::inventory::{ConfiguredInventoryEntry, configured_identity};
use systemprompt_marketplace::managed::{
    AssetDigest, AssetFile, NewResource, NewRevision, ResourceKind, RevisionFiles,
    SnapshotProvenance, SourceSpec,
};
use systemprompt_models::RequestContext;
use systemprompt_models::feedback::inventory::InventoryAvailability;
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context, fixture_app_context_with, fixture_db_pool,
    seed_user_row,
};
use tower::ServiceExt;

struct Harness {
    app: axum::Router,
    ctx: AppContext,
}

async fn seeded_revision(
    repository: &systemprompt_marketplace::managed::ManagedRepository,
    owner: &systemprompt_identifiers::UserId,
    key: &str,
    bytes: &[u8],
) -> (
    systemprompt_identifiers::ManagedResourceId,
    systemprompt_identifiers::ResourceRevisionId,
) {
    let source = repository
        .register_source(owner, key, &SourceSpec::Managed)
        .await
        .unwrap();
    let snapshot = repository
        .capture_snapshot(
            owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".into(),
                commit: None,
                tree_digest: AssetDigest::of(bytes),
                importer_version: "route-lifecycle".into(),
            },
        )
        .await
        .unwrap();
    let resource = repository
        .bind_resource(
            owner,
            &NewResource {
                source_id: source,
                upstream_key: key.into(),
                kind: ResourceKind::Skill,
                resource_key: key.into(),
            },
        )
        .await
        .unwrap();
    let revision = repository
        .create_revision(
            owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: RevisionFiles(BTreeMap::from([(
                    "SKILL.md".into(),
                    AssetFile {
                        bytes: bytes.to_vec(),
                        media_type: "text/markdown".into(),
                        executable: false,
                    },
                )])),
                dependencies: BTreeMap::new(),
                rationale: "route lifecycle fixture".into(),
            },
        )
        .await
        .unwrap();
    (resource, revision)
}

#[tokio::test]
async fn revision_bundle_and_git_binding_routes_preserve_owner_and_inventory_scope() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let owned_key = format!("owned-{suffix}");
    let (resource, revision) =
        seeded_revision(repository, &owner, &owned_key, b"# owner-only revision\n").await;

    let bundle = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/revisions/{revision}/bundle"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bundle.status(), StatusCode::OK);
    let bundle = json(bundle).await;
    let digest = bundle["revisions"][revision.as_str()]["files"]["SKILL.md"]["digest"]
        .as_str()
        .expect("bundle file digest");
    let encoded = bundle["assets"][digest]
        .as_array()
        .expect("serialized file bytes");
    let bytes: Vec<u8> = encoded
        .iter()
        .map(|byte| byte.as_u64().unwrap() as u8)
        .collect();
    assert_eq!(bytes, b"# owner-only revision\n");

    let foreign = systemprompt_identifiers::UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(
        harness.ctx.db_pool(),
        &foreign,
        &format!("{foreign}@foreign.invalid"),
    )
    .await
    .unwrap();
    let (foreign_resource, foreign_revision) = seeded_revision(
        repository,
        &foreign,
        &format!("foreign-{suffix}"),
        b"# foreign secret\n",
    )
    .await;
    let denied = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/revisions/{foreign_revision}/bundle"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::NOT_FOUND);
    let denied_body = to_bytes(denied.into_body(), 64 * 1024).await.unwrap();
    assert!(
        !denied_body
            .windows(b"foreign secret".len())
            .any(|w| w == b"foreign secret")
    );
    assert!(!String::from_utf8_lossy(&denied_body).contains(foreign_resource.as_str()));

    let entry = ConfiguredInventoryEntry {
        kind: "skill".into(),
        resource_key: owned_key.clone(),
        relative_root: format!("skills/{owned_key}"),
        availability: InventoryAvailability::Available,
        diagnostic: None,
    };
    let unbound_key = format!("unbound-{suffix}");
    let unbound_entry = ConfiguredInventoryEntry {
        kind: "skill".into(),
        resource_key: unbound_key.clone(),
        relative_root: format!("skills/{unbound_key}"),
        availability: InventoryAvailability::Available,
        diagnostic: None,
    };
    repository
        .reconcile_inventory(&owner, &[entry, unbound_entry])
        .await
        .unwrap();
    let entry_id = configured_identity(&owner, "skill", &owned_key);
    let unbound_entry_id = configured_identity(&owner, "skill", &unbound_key);
    let actor = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::try_new("binding-lifecycle").unwrap(),
    )
    .with_actor(systemprompt_identifiers::Actor::user(owner.clone()));
    let inventory_bound = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/inventory/{entry_id}/bindings"))
                .header("content-type", "application/json")
                .extension(actor.clone())
                .body(Body::from(
                    serde_json::json!({"resource_id": resource}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inventory_bound.status(), StatusCode::NO_CONTENT);
    let git_source = repository
        .register_source(
            &owner,
            &format!("git-{suffix}"),
            &SourceSpec::Git {
                repository: "https://example.invalid/catalog.git".into(),
                reference: "main".into(),
                subdirectory: None,
                credential_reference: None,
            },
        )
        .await
        .unwrap();

    let before = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/inventory/{entry_id}/git-binding"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::OK);
    assert!(json(before).await.is_null());

    let bound = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/sources/{git_source}/verification-bindings"))
                .header("content-type", "application/json")
                .extension(actor)
                .body(Body::from(
                    serde_json::json!({
                        "resource_id": resource,
                        "relative_root": format!("skills/{owned_key}")
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bound.status(), StatusCode::NO_CONTENT);
    let after = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/inventory/{entry_id}/git-binding"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(after.status(), StatusCode::OK);
    let after = json(after).await;
    assert_eq!(after["source_id"], git_source.as_str());
    assert_eq!(after["relative_root"], format!("skills/{owned_key}"));
    assert_eq!(after["bound_by"], owner.as_str());
    let unbound = harness
        .app
        .oneshot(
            Request::builder()
                .uri(format!("/inventory/{unbound_entry_id}/git-binding"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unbound.status(), StatusCode::OK);
    assert!(
        json(unbound).await.is_null(),
        "binding must remain entry-scoped"
    );
}

#[tokio::test]
async fn administrative_consumer_grant_can_be_enabled_and_revoked_without_losing_history() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let (resource, _) = seeded_revision(
        harness.ctx.managed_repository(),
        &owner,
        &format!("consumer-grant-{suffix}"),
        b"# Consumer grant lifecycle\n",
    )
    .await;
    let consumer = systemprompt_identifiers::UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(
        harness.ctx.db_pool(),
        &consumer,
        &format!("{consumer}@consumer-grant.invalid"),
    )
    .await
    .expect("consumer user");
    let request = |enabled| {
        Request::builder()
            .method("POST")
            .uri(format!("/resources/{resource}/consumer-grants"))
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::json!({
                    "consumer_id": consumer.as_str(),
                    "enabled": enabled
                })
                .to_string(),
            ))
            .unwrap()
    };

    let enabled = harness.app.clone().oneshot(request(true)).await.unwrap();
    assert_eq!(enabled.status(), StatusCode::NO_CONTENT);
    let database = harness.ctx.db_pool().pool_arc().expect("read pool");
    let active: Option<bool> = sqlx::query_scalar(
        "SELECT revoked_at IS NULL FROM managed_consumer_grants \
         WHERE owner_id=$1 AND resource_id=$2 AND consumer_id=$3",
    )
    .bind(owner.as_str())
    .bind(resource.as_str())
    .bind(consumer.as_str())
    .fetch_optional(database.as_ref())
    .await
    .unwrap();
    assert_eq!(active, Some(true));

    let revoked = harness.app.oneshot(request(false)).await.unwrap();
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);
    let active: Option<bool> = sqlx::query_scalar(
        "SELECT revoked_at IS NULL FROM managed_consumer_grants \
         WHERE owner_id=$1 AND resource_id=$2 AND consumer_id=$3",
    )
    .bind(owner.as_str())
    .bind(resource.as_str())
    .bind(consumer.as_str())
    .fetch_optional(database.as_ref())
    .await
    .unwrap();
    assert_eq!(active, Some(false), "revocation retains the audit row");
}

#[tokio::test]
async fn dependency_verification_failure_is_retained_and_replayed_without_git_access() {
    ensure_test_bootstrap();
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("api_dependency_verification_failure")
            .await
            .expect("private verification database");
    let db = database.pool().await.expect("private pool");
    let ctx = fixture_app_context(&db, database.url()).expect("private context");
    let owner = ctx.system_admin().id().clone();
    seed_user_row(&db, &owner, &format!("{owner}@verification-route.invalid"))
        .await
        .expect("administrative owner");
    let missing_reference = format!("missing-route-secret-{}", uuid::Uuid::new_v4().simple());
    let source = ctx
        .managed_repository()
        .register_source(
            &owner,
            "private-verification-source",
            &SourceSpec::Git {
                repository: "https://git.example.invalid/private.git".to_owned(),
                reference: "main".to_owned(),
                subdirectory: None,
                credential_reference: Some(missing_reference.clone()),
            },
        )
        .await
        .expect("registered Git source");
    let revision = systemprompt_identifiers::ResourceRevisionId::generate();
    let input = systemprompt_models::feedback::verification::DependencyVerificationRequest {
        root_revision_id: revision.clone(),
        revisions: vec![
            systemprompt_models::feedback::verification::DependencyVerificationInput {
                revision_id: revision,
                source_id: source,
                exact_commit: "a".repeat(40),
                relative_root: "skill".to_owned(),
                dependencies: Vec::new(),
            },
        ],
    };
    let key = systemprompt_identifiers::TaskId::generate();
    let request = || {
        Request::builder()
            .method("POST")
            .uri("/source-verifications")
            .header("content-type", "application/json")
            .header("idempotency-key", key.as_str())
            .body(Body::from(serde_json::to_vec(&input).unwrap()))
            .unwrap()
    };
    let app = systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));

    let first = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(first.status(), StatusCode::CONFLICT);
    let first_body = to_bytes(first.into_body(), 64 * 1024).await.unwrap();
    let first_body = String::from_utf8_lossy(&first_body);
    assert!(first_body.contains("Git credential"), "{first_body}");
    assert!(!first_body.contains(&missing_reference), "{first_body}");
    let failed = ctx
        .managed_repository()
        .api_operation(&owner, &key)
        .await
        .expect("failed operation retained");
    assert_eq!(failed.state, "failed");
    assert!(failed.result.is_none());

    let replay = app.oneshot(request()).await.unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["operation"]["id"], key.as_str());
    assert_eq!(replay["operation"]["state"], "failed");
    assert!(replay["result"].is_null());
    assert_eq!(
        replay["operation"]["updated_at"],
        serde_json::to_value(failed.updated_at).unwrap(),
        "retained replay does not re-run Git verification"
    );

    drop(ctx);
    drop(db);
    database.drop_now().await;
}

#[tokio::test]
async fn analytics_status_reports_only_the_administrative_owners_pending_jobs() {
    use systemprompt_analytics::snapshots::SnapshotRangeRequest;
    use systemprompt_identifiers::AnalyticsSnapshotJobId;

    ensure_test_bootstrap();
    let database = systemprompt_test_fixtures::DisposableDb::installed("api_snapshot_health")
        .await
        .expect("private snapshot database");
    let db = database.pool().await.expect("private pool");
    let ctx = fixture_app_context(&db, database.url()).expect("private context");
    let owner = ctx.system_admin().id().clone();
    seed_user_row(&db, &owner, &format!("{owner}@snapshot-health.invalid"))
        .await
        .expect("owner");
    let now = chrono::Utc::now();
    let request = SnapshotRangeRequest {
        operation_id: AnalyticsSnapshotJobId::generate(),
        resource_id: None,
        from_day: now.date_naive() - chrono::Duration::days(1),
        to_day: now.date_naive(),
    };
    ctx.feedback_snapshots_repository()
        .request_range(&owner, &request, now)
        .await
        .expect("pending range job");
    let foreign = systemprompt_identifiers::UserId::new(uuid::Uuid::new_v4().to_string());
    seed_user_row(&db, &foreign, &format!("{foreign}@snapshot-health.invalid"))
        .await
        .expect("foreign owner");
    let foreign_request = SnapshotRangeRequest {
        operation_id: AnalyticsSnapshotJobId::generate(),
        ..request
    };
    ctx.feedback_snapshots_repository()
        .request_range(&foreign, &foreign_request, now)
        .await
        .expect("foreign pending range job");
    let app = systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/analytics/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let health: systemprompt_analytics::snapshots::SnapshotHealth =
        serde_json::from_value(json(response).await).unwrap();
    assert_eq!(
        health.pending_jobs, 1,
        "foreign work must not affect owner health"
    );
    assert_eq!(health.pending_changes, 0);
    assert_eq!(health.pending_producer_changes, 0);

    drop(ctx);
    drop(db);
    database.drop_now().await;
}

async fn router() -> Harness {
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url)
        .await
        .expect("test database");
    let ctx = fixture_app_context(&db, &bootstrap.database_url).expect("fixture context");
    let owner = ctx.system_admin().id();
    seed_user_row(&db, owner, &format!("{owner}@managed-workflows.invalid"))
        .await
        .expect("administrative owner");
    let app = systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));
    Harness {
        app,
        ctx: ctx.as_ref().clone(),
    }
}

async fn json(response: axum::response::Response) -> serde_json::Value {
    let body = to_bytes(response.into_body(), 64 * 1024)
        .await
        .expect("response body");
    serde_json::from_slice(&body).expect("JSON response")
}

#[tokio::test]
async fn configured_root_refresh_tracks_diagnostics_recovery_and_withdrawal_history() {
    ensure_test_bootstrap();
    let database = systemprompt_test_fixtures::DisposableDb::installed("api_inventory_scan")
        .await
        .expect("isolated inventory database");
    let db = database.pool().await.expect("isolated inventory pool");
    let database_url = database.url().to_owned();
    let root = tempfile::tempdir().expect("isolated services root");
    let paths = PathsConfig {
        system: root.path().to_string_lossy().into_owned(),
        services: root.path().to_string_lossy().into_owned(),
        bin: root.path().join("bin").to_string_lossy().into_owned(),
        web_path: Some(root.path().join("web").to_string_lossy().into_owned()),
        storage: Some(root.path().join("storage").to_string_lossy().into_owned()),
        geoip_database: None,
    };
    let ctx = fixture_app_context_with(
        &db,
        &database_url,
        paths,
        Arc::new(systemprompt_marketplace::AllowAllFilter),
    )
    .expect("isolated context");
    let owner = ctx.system_admin().id().clone();
    seed_user_row(&db, &owner, &format!("{owner}@inventory-scan.invalid"))
        .await
        .expect("administrative owner");
    let app = systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));
    let valid_id = format!("scan-valid-{}", uuid::Uuid::new_v4().simple());
    let invalid_id = format!("scan-invalid-{}", uuid::Uuid::new_v4().simple());
    let valid = root.path().join("skills").join(&valid_id);
    let invalid = root.path().join("skills").join(&invalid_id);
    std::fs::create_dir_all(&valid).expect("valid skill directory");
    std::fs::create_dir_all(&invalid).expect("invalid skill directory");
    std::fs::write(
        valid.join("config.yaml"),
        format!("id: {valid_id}\nname: Valid scan\ndescription: inventory fixture\n"),
    )
    .expect("valid configuration");
    std::fs::write(valid.join("index.md"), "# First instructions\n").expect("valid content");
    std::fs::write(
        invalid.join("config.yaml"),
        "id: conflicts-with-directory\nname: Invalid scan\ndescription: inventory fixture\n",
    )
    .expect("invalid configuration");
    std::fs::write(invalid.join("index.md"), "# Initially invalid\n").expect("invalid content");

    let refresh = |key: &systemprompt_identifiers::TaskId| {
        Request::builder()
            .method("POST")
            .uri("/inventory/reconciliations")
            .header("idempotency-key", key.as_str())
            .body(Body::empty())
            .expect("refresh request")
    };
    let first_key = systemprompt_identifiers::TaskId::generate();
    let first = app
        .clone()
        .oneshot(refresh(&first_key))
        .await
        .expect("first refresh");
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert_eq!(first["operation"]["state"], "completed");
    let first_at = first["result"]["observed_at"]
        .as_str()
        .expect("observation timestamp")
        .to_owned();
    let valid_entry = configured_identity(&owner, "skill", &valid_id);
    let invalid_entry = configured_identity(&owner, "skill", &invalid_id);
    let valid_read = json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/inventory/{valid_entry}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(valid_read["availability"], "available");
    assert_eq!(valid_read["configured_key"], format!("skills/{valid_id}"));
    let invalid_read = json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/inventory/{invalid_entry}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(invalid_read["availability"], "unavailable");
    assert!(
        invalid_read["diagnostic"]
            .as_str()
            .is_some_and(|message| message.contains("identity conflicts"))
    );
    let baseline_request = |operation: &systemprompt_identifiers::TaskId| {
        Request::builder()
            .method("POST")
            .uri("/inventory/baselines")
            .header("content-type", "application/json")
            .extension(
                RequestContext::new(
                    SessionId::generate(),
                    TraceId::generate(),
                    ContextId::generate(),
                    AgentName::try_new("inventory-scan-test").expect("agent name"),
                )
                .with_actor(systemprompt_identifiers::Actor::user(owner.clone())),
            )
            .body(Body::from(
                serde_json::json!({"operation_id": operation, "after": null, "limit": 100})
                    .to_string(),
            ))
            .expect("baseline request")
    };
    let first_baseline_operation = systemprompt_identifiers::TaskId::generate();
    let first_baselines = json(
        app.clone()
            .oneshot(baseline_request(&first_baseline_operation))
            .await
            .expect("first baseline response"),
    )
    .await;
    let first_revision = first_baselines["items"]
        .as_array()
        .expect("baseline items")
        .iter()
        .find(|item| item["entry_id"] == valid_entry.as_str())
        .and_then(|item| item["revision_id"].as_str())
        .expect("valid entry baseline revision")
        .to_owned();
    let first_revision_id = systemprompt_identifiers::ResourceRevisionId::new(&first_revision);
    let first_manifest = ctx
        .managed_repository()
        .get_revision(&owner, &first_revision_id)
        .await
        .expect("baseline manifest");
    let valid_resource = ctx
        .managed_repository()
        .revision_resource(&owner, &first_revision_id)
        .await
        .expect("baseline resource");
    let mut candidate_files = ctx
        .managed_repository()
        .get_revision_files(&owner, &first_revision_id)
        .await
        .expect("baseline files");
    candidate_files.0.get_mut("index.md").unwrap().bytes = b"# unpublished candidate\n".to_vec();
    let candidate = ctx
        .managed_repository()
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: valid_resource,
                snapshot_id: first_manifest.snapshot_id,
                parent_id: Some(first_revision_id),
                files: candidate_files,
                dependencies: first_manifest.dependencies,
                rationale: "unpublished candidate before configured edit".into(),
            },
        )
        .await
        .expect("candidate revision");

    std::fs::write(
        invalid.join("config.yaml"),
        format!("id: {invalid_id}\nname: Recovered scan\ndescription: inventory fixture\n"),
    )
    .expect("repaired configuration");
    std::fs::write(valid.join("index.md"), "# Changed instructions\n").expect("changed content");
    let second_key = systemprompt_identifiers::TaskId::generate();
    let second = app
        .clone()
        .oneshot(refresh(&second_key))
        .await
        .expect("recovery refresh");
    assert_eq!(second.status(), StatusCode::OK);
    let second = json(second).await;
    assert!(second["result"]["generation"].as_i64() > first["result"]["generation"].as_i64());
    let second_baseline_operation = systemprompt_identifiers::TaskId::generate();
    let second_baselines = json(
        app.clone()
            .oneshot(baseline_request(&second_baseline_operation))
            .await
            .expect("changed baseline response"),
    )
    .await;
    let changed_capture = second_baselines["items"]
        .as_array()
        .expect("changed baseline items")
        .iter()
        .find(|item| item["entry_id"] == valid_entry.as_str())
        .expect("changed baseline capture");
    assert_eq!(changed_capture["status"], "reconciliation_required");
    let second_revision = changed_capture["revision_id"]
        .as_str()
        .expect("changed baseline revision");
    let reconciliation = changed_capture["reconciliation_id"]
        .as_str()
        .expect("linked reconciliation");
    assert_ne!(second_revision, first_revision);
    assert_ne!(second_revision, candidate.as_str());
    let changed_files = ctx
        .managed_repository()
        .get_revision_files(
            &owner,
            &systemprompt_identifiers::ResourceRevisionId::new(second_revision),
        )
        .await
        .expect("changed configured files");
    assert_eq!(
        changed_files.0["index.md"].bytes,
        b"# Changed instructions\n"
    );
    let capture_read = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/inventory/{valid_entry}/captures/{second_baseline_operation}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(capture_read.status(), StatusCode::OK);
    let capture_read = json(capture_read).await;
    assert_eq!(capture_read["revision_id"], second_revision);
    assert_eq!(capture_read["reconciliation_id"], reconciliation);
    let reconciliations = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/inventory/{valid_entry}/reconciliations"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reconciliations.status(), StatusCode::OK);
    let reconciliations = json(reconciliations).await;
    let linked = reconciliations["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == reconciliation)
        .expect("capture reconciliation appears in route listing");
    assert_eq!(linked["upstream_base_revision_id"], first_revision);
    assert_eq!(linked["managed_candidate_revision_id"], candidate.as_str());
    assert_eq!(linked["incoming_revision_id"], second_revision);

    let recovered = json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/inventory/{invalid_entry}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(recovered["availability"], "available");
    assert_eq!(recovered["diagnostic"], serde_json::Value::Null);

    std::fs::remove_dir_all(&invalid).expect("remove recovered configured skill");
    let third_key = systemprompt_identifiers::TaskId::generate();
    let third = app
        .clone()
        .oneshot(refresh(&third_key))
        .await
        .expect("withdrawal refresh");
    assert_eq!(third.status(), StatusCode::OK);
    let withdrawn = json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/inventory/{invalid_entry}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(withdrawn["availability"], "withdrawn");
    assert_eq!(
        withdrawn["diagnostic"],
        "Configured source entry was removed; retained publication is unchanged"
    );

    let historic = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/inventory/{invalid_entry}/membership?at={first_at}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(historic.status(), StatusCode::OK);
    let historic = json(historic).await;
    assert_eq!(historic["status"], "known");
    assert_eq!(historic["entry"]["availability"], "unavailable");
    assert!(historic["effective_until"].as_str().is_some());

    drop(app);
    drop(ctx);
    db.write_pool_arc()
        .expect("isolated write pool")
        .close()
        .await;
    database.drop_now().await;
}

#[tokio::test]
async fn managed_source_lifecycle_persists_and_missing_reads_are_problem_details() {
    let harness = router().await;
    let app = harness.app;
    let create = Request::builder()
        .method("POST")
        .uri("/sources")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "name": "HTTP-managed-authoring",
                "specification": {"kind": "managed"}
            })
            .to_string(),
        ))
        .expect("create source request");
    let created = app
        .clone()
        .oneshot(create)
        .await
        .expect("create source response");
    assert_eq!(created.status(), StatusCode::CREATED);
    let location = created.headers()["location"]
        .to_str()
        .expect("location")
        .to_owned();
    let source = json(created).await;
    let source_id = source.as_str().expect("source id");
    assert!(location.ends_with(source_id));

    let stored = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/sources/{source_id}"))
                .body(Body::empty())
                .expect("source read request"),
        )
        .await
        .expect("source read response");
    assert_eq!(stored.status(), StatusCode::OK);
    assert_eq!(json(stored).await["kind"], "managed");

    let missing = app
        .oneshot(
            Request::builder()
                .uri("/sources/not-a-source")
                .body(Body::empty())
                .expect("missing source request"),
        )
        .await
        .expect("missing source response");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        missing.headers()["content-type"],
        "application/problem+json"
    );
}

#[tokio::test]
async fn reviewed_publication_is_idempotent_and_retains_auditable_history() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let source = repository
        .register_source(&owner, "review-http", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            &owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"review-http"),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("snapshot");
    let resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source.clone(),
                upstream_key: "review-http".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "review-http".to_owned(),
            },
        )
        .await
        .expect("resource");
    let revision = repository
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: RevisionFiles(BTreeMap::from([(
                    "SKILL.md".to_owned(),
                    AssetFile {
                        bytes: b"# Reviewed HTTP skill".to_vec(),
                        media_type: "text/markdown".to_owned(),
                        executable: false,
                    },
                )])),
                dependencies: BTreeMap::new(),
                rationale: "review fixture".to_owned(),
            },
        )
        .await
        .expect("revision");
    let operation_key = format!("review-http-{}", uuid::Uuid::new_v4().simple());
    let request = || {
        Request::builder()
            .method("POST")
            .uri("/publications")
            .header("content-type", "application/json")
            .extension(
                RequestContext::new(
                    SessionId::generate(),
                    TraceId::generate(),
                    ContextId::generate(),
                    AgentName::try_new("publication-test").expect("agent name"),
                )
                .with_actor(systemprompt_identifiers::Actor::user(owner.clone())),
            )
            .body(Body::from(
                serde_json::json!({
                    "resource_id": resource.clone(),
                    "revision_id": revision.clone(),
                    "action": "initial_adoption",
                    "expected_generation": 0,
                    "operation_key": operation_key.clone(),
                    "comparison_evidence": {"reviewer_note": "checked canonical files"},
                    "limitations": "validated by HTTP lifecycle fixture"
                })
                .to_string(),
            ))
            .expect("publication request")
    };
    let first = harness
        .app
        .clone()
        .oneshot(request())
        .await
        .expect("publication response");
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert_eq!(first["resource_id"], resource.as_str());
    assert_eq!(first["revision_id"], revision.as_str());
    assert_eq!(first["generation"], 1);
    assert_eq!(first["action"], "initial_adoption");

    let replay = harness
        .app
        .clone()
        .oneshot(request())
        .await
        .expect("publication replay");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["publication_id"], first["publication_id"]);
    assert_eq!(replay["review_id"], first["review_id"]);

    let history = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/resources/{resource}/publications?limit=1"))
                .body(Body::empty())
                .expect("publication history request"),
        )
        .await
        .expect("publication history response");
    assert_eq!(history.status(), StatusCode::OK);
    let history = json(history).await;
    assert_eq!(history["items"].as_array().expect("history").len(), 1);
    assert_eq!(
        history["items"][0]["decision"]["publication_id"],
        first["publication_id"]
    );
    assert_eq!(history["items"][0]["reviewer_id"], owner.as_str());
    assert_eq!(
        history["items"][0]["comparison_evidence"]["reviewer_note"],
        "checked canonical files"
    );
    assert_eq!(
        history["items"][0]["limitations"],
        "validated by HTTP lifecycle fixture"
    );

    let stale = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/publications")
                .header("content-type", "application/json")
                .extension(
                    RequestContext::new(
                        SessionId::generate(),
                        TraceId::generate(),
                        ContextId::generate(),
                        AgentName::try_new("publication-test").expect("agent name"),
                    )
                    .with_actor(systemprompt_identifiers::Actor::user(owner.clone())),
                )
                .body(Body::from(
                    serde_json::json!({
                        "resource_id": resource.clone(),
                        "revision_id": revision.clone(),
                        "action": "initial_adoption",
                        "expected_generation": 0,
                        "operation_key": format!("stale-review-{}", uuid::Uuid::new_v4().simple()),
                        "comparison_evidence": {"reviewer_note": "stale attempt"},
                        "limitations": "must not replace the reviewed publication"
                    })
                    .to_string(),
                ))
                .expect("stale publication request"),
        )
        .await
        .expect("stale publication response");
    assert_eq!(stale.status(), StatusCode::CONFLICT);

    let withdrawal = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/publications")
                .header("content-type", "application/json")
                .extension(
                    RequestContext::new(
                        SessionId::generate(),
                        TraceId::generate(),
                        ContextId::generate(),
                        AgentName::try_new("publication-test").expect("agent name"),
                    )
                    .with_actor(systemprompt_identifiers::Actor::user(owner.clone())),
                )
                .body(Body::from(
                    serde_json::json!({
                        "resource_id": resource.clone(),
                        "revision_id": serde_json::Value::Null,
                        "action": "withdraw",
                        "expected_generation": 1,
                        "operation_key": format!("withdraw-review-{}", uuid::Uuid::new_v4().simple()),
                        "comparison_evidence": {"reviewer_note": "withdrawn after review"},
                        "limitations": "distribution intentionally stopped"
                    })
                    .to_string(),
                ))
                .expect("withdrawal request"),
        )
        .await
        .expect("withdrawal response");
    assert_eq!(withdrawal.status(), StatusCode::OK);
    let withdrawal = json(withdrawal).await;
    assert_eq!(withdrawal["generation"], 2);
    assert_eq!(withdrawal["action"], "withdraw");
    assert_eq!(withdrawal["revision_id"], serde_json::Value::Null);
    let history = harness
        .app
        .oneshot(
            Request::builder()
                .uri(format!("/resources/{resource}/publications?limit=10"))
                .body(Body::empty())
                .expect("publication history after withdrawal"),
        )
        .await
        .expect("publication history after withdrawal response");
    assert_eq!(history.status(), StatusCode::OK);
    let history = json(history).await;
    let history = history["items"].as_array().expect("two immutable reviews");
    assert_eq!(history.len(), 2);
    assert!(history.iter().any(|entry| {
        entry["decision"]["publication_id"] == first["publication_id"]
            && entry["decision"]["action"] == "initial_adoption"
            && entry["comparison_evidence"]["reviewer_note"] == "checked canonical files"
    }));
    assert!(history.iter().any(|entry| {
        entry["decision"]["publication_id"] == withdrawal["publication_id"]
            && entry["decision"]["action"] == "withdraw"
            && entry["comparison_evidence"]["reviewer_note"] == "withdrawn after review"
    }));
}

#[tokio::test]
async fn source_capture_failure_is_retained_for_a_valid_idempotent_operation() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let source = harness
        .ctx
        .managed_repository()
        .register_source(&owner, "empty-authoring", &SourceSpec::Managed)
        .await
        .expect("managed source");
    let operation = systemprompt_identifiers::TaskId::generate();
    let capture = || {
        Request::builder()
            .method("POST")
            .uri(format!("/sources/{source}/captures"))
            .header("content-type", "application/json")
            .header("idempotency-key", operation.as_str())
            .body(Body::from(
                serde_json::json!({"skill_ids":["missing-skill"]}).to_string(),
            ))
            .expect("capture request")
    };
    let failed = harness
        .app
        .clone()
        .oneshot(capture())
        .await
        .expect("capture failure response");
    assert_eq!(failed.status(), StatusCode::CONFLICT);
    let failed = json(failed).await;
    assert!(
        failed["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("local-tree source")),
        "the authoring-source requirement is actionable: {failed}"
    );
    let status = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/operations/{operation}"))
                .body(Body::empty())
                .expect("operation status request"),
        )
        .await
        .expect("operation status response");
    assert_eq!(status.status(), StatusCode::OK);
    let status = json(status).await;
    assert_eq!(status["operation"]["kind"], "source_capture");
    assert_eq!(status["operation"]["state"], "failed");
    assert_eq!(
        status["operation"]["problem"],
        "Review retained inputs and use a new operation key after correcting the failure"
    );

    let replay = harness
        .app
        .oneshot(capture())
        .await
        .expect("failed operation replay");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["operation"]["state"], "failed");
    assert_eq!(replay["result"], serde_json::Value::Null);
    let replay_status = harness
        .ctx
        .managed_repository()
        .api_operation(&owner, &operation)
        .await
        .expect("retained failure");
    assert_eq!(replay_status.state, "failed");
    assert!(replay_status.result.is_none());
}

#[tokio::test]
async fn malformed_capture_input_is_rejected_before_creating_an_idempotent_operation() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let source = harness
        .ctx
        .managed_repository()
        .register_source(&owner, "capture-shape", &SourceSpec::Managed)
        .await
        .expect("managed source");
    let operation = systemprompt_identifiers::TaskId::generate();

    let response = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/sources/{source}/captures"))
                .header("content-type", "application/json")
                .header("idempotency-key", operation.as_str())
                .body(Body::from(serde_json::json!({"skill_ids": []}).to_string()))
                .expect("capture request"),
        )
        .await
        .expect("capture response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let problem = json(response).await;
    assert_eq!(problem["status"], StatusCode::BAD_REQUEST.as_u16());
    assert!(
        problem["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("1–100 bounded skill identifiers")),
        "the client receives the bounded capture contract: {problem}"
    );

    let database = harness.ctx.db_pool().pool_arc().expect("read pool");
    let rows: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM managed_api_operations WHERE owner_id = $1 AND id = $2",
        owner.as_str(),
        operation.as_str(),
    )
    .fetch_one(database.as_ref())
    .await
    .expect("operation count")
    .unwrap_or(0);
    assert_eq!(
        rows, 0,
        "input validation must run before idempotency allocation so malformed retries leave no retained operation"
    );
}

#[tokio::test]
async fn local_tree_capture_commits_revision_clears_checkpoint_and_replays_completed_operation() {
    struct FixtureDir(std::path::PathBuf);
    impl Drop for FixtureDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let root = harness.ctx.app_paths().system().services().to_path_buf();
    let skill_id = format!("capture-{}", uuid::Uuid::new_v4().simple());
    let skill = root.join("skills").join(&skill_id);
    std::fs::create_dir_all(&skill).expect("isolated authoring skill");
    let fixture = FixtureDir(skill);
    std::fs::write(
        fixture.0.join("config.yaml"),
        format!("id: {skill_id}\nname: Local capture\ndescription: API capture fixture\n"),
    )
    .expect("skill configuration");
    std::fs::write(fixture.0.join("index.md"), "# Durable local instructions\n")
        .expect("skill instructions");
    let source = repository
        .register_source(
            &owner,
            &format!("local-capture-{skill_id}"),
            &SourceSpec::LocalTree {
                root: root.to_string_lossy().into_owned(),
            },
        )
        .await
        .expect("configured local source");
    let operation = systemprompt_identifiers::TaskId::generate();
    let request = || {
        Request::builder()
            .method("POST")
            .uri(format!("/sources/{source}/captures"))
            .header("content-type", "application/json")
            .header("idempotency-key", operation.as_str())
            .body(Body::from(
                serde_json::json!({"skill_ids":[skill_id.clone()]}).to_string(),
            ))
            .expect("capture request")
    };

    let response = harness
        .app
        .clone()
        .oneshot(request())
        .await
        .expect("capture response");
    assert_eq!(response.status(), StatusCode::OK);
    let first = json(response).await;
    assert_eq!(first["operation"]["state"], "completed");
    assert_eq!(first["result"]["source_id"], source.as_str());
    let revision = systemprompt_identifiers::ResourceRevisionId::new(
        first["result"]["revisions"]
            .get(&skill_id)
            .and_then(serde_json::Value::as_str)
            .expect("captured revision"),
    );
    let files = repository
        .get_revision_files(&owner, &revision)
        .await
        .expect("durable revision files");
    assert_eq!(files.0["index.md"].bytes, b"# Durable local instructions\n");
    assert_eq!(files.0["config.yaml"].media_type, "application/yaml");
    let retained = repository
        .api_operation(&owner, &operation)
        .await
        .expect("completed operation");
    assert_eq!(retained.state, "completed");
    assert_eq!(retained.result.as_ref(), Some(&first["result"]));
    assert!(
        repository
            .api_input::<systemprompt_marketplace::managed::CapturedSkills>(&owner, &retained)
            .await
            .expect("checkpoint lookup")
            .is_none(),
        "completion clears restart-only input after retaining the terminal result"
    );

    std::fs::write(fixture.0.join("index.md"), "# Changed after completion\n")
        .expect("post-completion source change");
    let replay = harness
        .app
        .oneshot(request())
        .await
        .expect("capture replay");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["operation"]["id"], operation.as_str());
    assert_eq!(replay["operation"]["state"], "completed");
    assert_eq!(replay["result"], first["result"]);
    let unchanged = repository
        .get_revision_files(&owner, &revision)
        .await
        .expect("retained revision after replay");
    assert_eq!(
        unchanged.0["index.md"].bytes,
        b"# Durable local instructions\n"
    );
}

#[tokio::test]
async fn inventory_refresh_persists_managed_membership_and_replays_its_operation() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let repository = harness.ctx.managed_repository();
    let source = repository
        .register_source(&owner, "inventory-http", &SourceSpec::Managed)
        .await
        .expect("source");
    let snapshot = repository
        .capture_snapshot(
            &owner,
            &source,
            &SnapshotProvenance {
                source_kind: "managed".to_owned(),
                commit: None,
                tree_digest: AssetDigest::of(b"inventory-http"),
                importer_version: "test".to_owned(),
            },
        )
        .await
        .expect("snapshot");
    let resource = repository
        .bind_resource(
            &owner,
            &NewResource {
                source_id: source.clone(),
                upstream_key: "inventory-http".to_owned(),
                kind: ResourceKind::Skill,
                resource_key: "inventory-http".to_owned(),
            },
        )
        .await
        .expect("resource");
    repository
        .create_revision(
            &owner,
            &NewRevision {
                resource_id: resource.clone(),
                snapshot_id: snapshot,
                parent_id: None,
                files: RevisionFiles(BTreeMap::from([(
                    "SKILL.md".to_owned(),
                    AssetFile {
                        bytes: b"# Inventory HTTP".to_vec(),
                        media_type: "text/markdown".to_owned(),
                        executable: false,
                    },
                )])),
                dependencies: BTreeMap::new(),
                rationale: "inventory route fixture".to_owned(),
            },
        )
        .await
        .expect("revision");

    let key = systemprompt_identifiers::TaskId::generate();
    let refresh = || {
        Request::builder()
            .method("POST")
            .uri("/inventory/reconciliations")
            .header("idempotency-key", key.as_str())
            .body(Body::empty())
            .expect("refresh request")
    };
    let first = harness
        .app
        .clone()
        .oneshot(refresh())
        .await
        .expect("refresh response");
    assert_eq!(first.status(), StatusCode::OK);
    let first = json(first).await;
    assert_eq!(first["operation"]["id"], key.as_str());
    assert_eq!(first["operation"]["state"], "completed");
    assert!(
        first["result"]["entries"]
            .as_i64()
            .is_some_and(|entries| entries >= 1),
        "the seeded resource contributes a persisted inventory member: {first}"
    );

    let entries = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/inventory?limit=10")
                .body(Body::empty())
                .expect("inventory request"),
        )
        .await
        .expect("inventory response");
    assert_eq!(entries.status(), StatusCode::OK);
    let entries = json(entries).await;
    let entry = entries["items"]
        .as_array()
        .expect("inventory items")
        .iter()
        .find(|item| item["resource_id"] == resource.as_str())
        .expect("seeded resource is a durable inventory member");
    assert_eq!(entry["resource_key"], "inventory-http");
    let entry_id = entry["entry_id"]
        .as_str()
        .expect("inventory entry id")
        .to_owned();
    let entry_read = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/inventory/{entry_id}"))
                .body(Body::empty())
                .expect("inventory entry request"),
        )
        .await
        .expect("inventory entry response");
    assert_eq!(entry_read.status(), StatusCode::OK);
    let entry_read = json(entry_read).await;
    assert_eq!(entry_read["resource_id"], resource.as_str());
    assert_eq!(entry_read["source_id"], source.as_str());
    assert_eq!(entry_read["kind"], "skill");

    let observed_at = chrono::Utc::now();
    let observed_query = observed_at.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
    let membership = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/inventory/{entry_id}/membership?at={observed_query}"
                ))
                .body(Body::empty())
                .expect("current membership request"),
        )
        .await
        .expect("current membership response");
    assert_eq!(membership.status(), StatusCode::OK);
    let membership = json(membership).await;
    assert_eq!(membership["status"], "known");
    assert_eq!(membership["entry"]["entry_id"], entry_id);
    assert_eq!(membership["entry"]["resource_id"], resource.as_str());

    let future = observed_at + chrono::Duration::minutes(5);
    let future_query = future.to_rfc3339_opts(chrono::SecondsFormat::Micros, true);
    let future_membership = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/inventory/{entry_id}/membership?at={future_query}"
                ))
                .body(Body::empty())
                .expect("future membership request"),
        )
        .await
        .expect("future membership response");
    assert_eq!(future_membership.status(), StatusCode::OK);
    assert_eq!(json(future_membership).await["status"], "unknown");

    let configured = ConfiguredInventoryEntry {
        kind: "skill".to_owned(),
        resource_key: "inventory-http".to_owned(),
        relative_root: "skills/inventory-http".to_owned(),
        availability: InventoryAvailability::Available,
        diagnostic: None,
    };
    repository
        .reconcile_inventory(&owner, &[configured])
        .await
        .expect("configured inventory observation");
    let configured_id = configured_identity(&owner, "skill", "inventory-http");
    let bound = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/inventory/{configured_id}/bindings"))
                .header("content-type", "application/json")
                .extension(
                    RequestContext::new(
                        SessionId::generate(),
                        TraceId::generate(),
                        ContextId::generate(),
                        AgentName::try_new("inventory-test").expect("agent name"),
                    )
                    .with_actor(systemprompt_identifiers::Actor::user(owner.clone())),
                )
                .body(Body::from(
                    serde_json::json!({"resource_id": resource.clone()}).to_string(),
                ))
                .expect("binding request"),
        )
        .await
        .expect("binding response");
    assert_eq!(bound.status(), StatusCode::NO_CONTENT);
    let bound_entry = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/inventory/{configured_id}"))
                .body(Body::empty())
                .expect("bound entry request"),
        )
        .await
        .expect("bound entry response");
    assert_eq!(bound_entry.status(), StatusCode::OK);
    let bound_entry = json(bound_entry).await;
    assert_eq!(bound_entry["entry_id"], configured_id.as_str());
    assert_eq!(bound_entry["resource_id"], resource.as_str());
    assert_eq!(bound_entry["configured_key"], "skills/inventory-http");

    let replay = harness
        .app
        .clone()
        .oneshot(refresh())
        .await
        .expect("replay response");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["operation"]["id"], first["operation"]["id"]);
    assert_eq!(replay["result"], first["result"]);
    let status = harness
        .app
        .oneshot(
            Request::builder()
                .uri(format!("/operations/{key}"))
                .body(Body::empty())
                .expect("operation status request"),
        )
        .await
        .expect("operation status response");
    assert_eq!(status.status(), StatusCode::OK);
    let status = json(status).await;
    assert_eq!(status["result"]["kind"], "inventory_refresh");
    assert_eq!(status["result"]["value"], first["result"]);
}

#[tokio::test]
async fn snapshot_routes_return_seeded_scoped_aggregates_and_durable_range_jobs() {
    let harness = router().await;
    let owner = harness.ctx.system_admin().id().clone();
    let scope_suffix = uuid::Uuid::new_v4().simple().to_string();
    let resource =
        systemprompt_identifiers::ManagedResourceId::new(format!("snapshot-page-a-{scope_suffix}"));
    let today = chrono::Utc::now().date_naive();
    let snapshot = FeedbackSnapshot {
        resource_id: Some(resource.clone()),
        generation: 7,
        fact_generation: 11,
        from_day: today - chrono::Duration::days(29),
        to_day: today,
        generated_at: chrono::Utc::now(),
        metrics: SnapshotMetrics {
            requests: 4,
            invocations: 3,
            input_tokens: 40,
            output_tokens: 12,
            ..SnapshotMetrics::default()
        },
        spend_by_currency: BTreeMap::from([("USD".to_owned(), 700_i128)]),
        distinct_users: Some(2),
        distinct_sessions: Some(3),
        histogram: LatencyHistogram::default(),
        related_spend_non_additive: false,
        suppressed_days: 0,
        historical_identity_available: true,
    };
    let bootstrap = ensure_test_bootstrap();
    let db = fixture_db_pool(&bootstrap.database_url)
        .await
        .expect("test database");
    let writer = db.write_pool_arc().expect("write pool");
    sqlx::query("INSERT INTO analytics_feedback_snapshots(owner_id,scope,window_days,generation,from_day,to_day,body) VALUES($1,$2,30,$3,$4,$5,$6)")
        .bind(owner.as_str()).bind(resource.as_str()).bind(snapshot.generation).bind(snapshot.from_day).bind(snapshot.to_day).bind(serde_json::to_value(&snapshot).unwrap()).execute(writer.as_ref()).await.expect("snapshot seed");
    let second_resource =
        systemprompt_identifiers::ManagedResourceId::new(format!("snapshot-page-b-{scope_suffix}"));
    let mut second_snapshot = snapshot.clone();
    second_snapshot.resource_id = Some(second_resource.clone());
    second_snapshot.generation = 8;
    second_snapshot.metrics.requests = 9;
    second_snapshot.spend_by_currency = BTreeMap::from([("EUR".to_owned(), 250_i128)]);
    sqlx::query("INSERT INTO analytics_feedback_snapshots(owner_id,scope,window_days,generation,from_day,to_day,body) VALUES($1,$2,30,$3,$4,$5,$6)")
        .bind(owner.as_str()).bind(second_resource.as_str()).bind(second_snapshot.generation).bind(second_snapshot.from_day).bind(second_snapshot.to_day).bind(serde_json::to_value(&second_snapshot).unwrap()).execute(writer.as_ref()).await.expect("second resource snapshot seed");
    let mut portfolio_snapshot = snapshot.clone();
    portfolio_snapshot.resource_id = None;
    portfolio_snapshot.generation = 9;
    portfolio_snapshot.metrics.requests = 13;
    portfolio_snapshot.spend_by_currency =
        BTreeMap::from([("EUR".to_owned(), 250_i128), ("USD".to_owned(), 700_i128)]);
    sqlx::query("INSERT INTO analytics_feedback_snapshots(owner_id,scope,window_days,generation,from_day,to_day,body) VALUES($1,$2,30,$3,$4,$5,$6)")
        .bind(owner.as_str()).bind("").bind(portfolio_snapshot.generation).bind(portfolio_snapshot.from_day).bind(portfolio_snapshot.to_day).bind(serde_json::to_value(&portfolio_snapshot).unwrap()).execute(writer.as_ref()).await.expect("portfolio snapshot seed");
    let response = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/analytics/snapshots/{resource}?days=30"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["metrics"]["requests"], 4);
    assert_eq!(body["spend_by_currency"]["USD"], 700);
    let page = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/analytics/snapshots?days=30&limit=1&after={resource}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    let page = json(page).await;
    assert_eq!(page["items"][0]["resource_id"], second_resource.as_str());
    assert_eq!(page["items"][0]["metrics"]["requests"], 9);
    let next_cursor = page["next_cursor"]
        .as_str()
        .expect("cursor after second scope");
    assert_eq!(next_cursor, second_resource.as_str());
    let next_page = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/analytics/snapshots?days=30&limit=1&after={next_cursor}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(next_page.status(), StatusCode::OK);
    let next_page = json(next_page).await;
    assert!(
        next_page["items"]
            .as_array()
            .expect("snapshot page")
            .iter()
            .all(|item| item["resource_id"] != resource.as_str()
                && item["resource_id"] != second_resource.as_str()),
        "the cursor excludes both already-consumed resource scopes"
    );
    let portfolio = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/analytics/snapshots/portfolio?days=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(portfolio.status(), StatusCode::OK);
    let portfolio = json(portfolio).await;
    assert_eq!(portfolio["resource_id"], serde_json::Value::Null);
    assert_eq!(portfolio["metrics"]["requests"], 13);
    assert_eq!(portfolio["spend_by_currency"]["EUR"], 250);
    let job = systemprompt_identifiers::AnalyticsSnapshotJobId::generate();
    let request = serde_json::json!({"operation_id": job.clone(), "resource_id": resource.clone(), "from_day": (today - chrono::Duration::days(2)), "to_day": today});
    let created = harness
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/analytics/jobs")
                .header("content-type", "application/json")
                .body(Body::from(request.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    let retained = harness
        .app
        .oneshot(
            Request::builder()
                .uri(format!("/analytics/jobs/{job}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retained.status(), StatusCode::OK);
    let retained = json(retained).await;
    assert_eq!(retained["operation_id"], job.as_str());
    assert_eq!(retained["state"], "pending");
    assert_eq!(retained["result"], serde_json::Value::Null);
    let stored = sqlx::query_as::<_, (String, chrono::NaiveDate, chrono::NaiveDate)>(
        "SELECT scope,from_day,to_day FROM analytics_snapshot_jobs WHERE owner_id=$1 AND job_id=$2",
    )
    .bind(owner.as_str())
    .bind(job.as_str())
    .fetch_one(writer.as_ref())
    .await
    .expect("durable range job");
    assert_eq!(stored.0, resource.as_str());
    assert_eq!(stored.1, today - chrono::Duration::days(2));
    assert_eq!(stored.2, today);
}

#[tokio::test]
async fn inventory_refresh_retains_a_failed_operation_without_mutation_then_repairs_with_a_new_key()
{
    let boot = systemprompt_test_fixtures::init_unloadable_services_bootstrap(
        "http://127.0.0.1",
        "mcp_servers: [not-a-map]\n",
    );
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("api_inventory_refresh_config_failure")
            .await
            .expect("private inventory database");
    let db = database.pool().await.expect("private inventory pool");
    let ctx = fixture_app_context(&db, database.url()).expect("private application context");
    let owner = ctx.system_admin().id().clone();
    seed_user_row(&db, &owner, &format!("{owner}@inventory-failure.invalid"))
        .await
        .expect("administrative owner");
    let app = systemprompt_api::routes::managed::router()
        .with_state(systemprompt_api::routes::managed::state::ManagedState::new(
            ctx.as_ref().clone(),
        ))
        .layer(axum::middleware::from_fn(
            systemprompt_api::routes::managed::contract::normalize,
        ));
    let before = ctx
        .managed_repository()
        .inventory_status(&owner)
        .await
        .expect("initial inventory status");
    let failed_key = systemprompt_identifiers::TaskId::generate();
    let request = |key: &systemprompt_identifiers::TaskId| {
        Request::builder()
            .method("POST")
            .uri("/inventory/reconciliations")
            .header("idempotency-key", key.as_str())
            .body(Body::empty())
            .expect("inventory refresh request")
    };

    let failed = app
        .clone()
        .oneshot(request(&failed_key))
        .await
        .expect("failed refresh response");
    assert_eq!(failed.status(), StatusCode::BAD_REQUEST);
    let failed_body = json(failed).await;
    assert!(
        failed_body["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("Configured inventory unavailable")),
        "configuration failure is returned with its operation boundary: {failed_body}"
    );
    let retained = ctx
        .managed_repository()
        .api_operation(&owner, &failed_key)
        .await
        .expect("failed refresh operation");
    assert_eq!(retained.state, "failed");
    assert!(retained.result.is_none());
    let after_failure = ctx
        .managed_repository()
        .inventory_status(&owner)
        .await
        .expect("inventory after failed refresh");
    assert_eq!(
        (
            after_failure.generation,
            after_failure.observed_at,
            after_failure.entries,
            after_failure.last_error,
        ),
        (
            before.generation,
            before.observed_at,
            before.entries,
            before.last_error,
        ),
        "configuration failure cannot mutate the durable inventory projection"
    );

    std::fs::write(boot.services_path.join("config/config.yaml"), "{}\n")
        .expect("repair services configuration");
    systemprompt_loader::ConfigLoader::reload().expect("reload repaired services configuration");

    let replay = app
        .clone()
        .oneshot(request(&failed_key))
        .await
        .expect("failed operation replay");
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["operation"]["id"], failed_key.as_str());
    assert_eq!(replay["operation"]["state"], "failed");
    assert!(replay["result"].is_null());
    assert_eq!(
        replay["operation"]["updated_at"],
        serde_json::to_value(retained.updated_at).expect("retained timestamp"),
        "repair does not silently rerun a failed idempotency key"
    );

    let repaired_key = systemprompt_identifiers::TaskId::generate();
    let repaired = app
        .clone()
        .oneshot(request(&repaired_key))
        .await
        .expect("repaired refresh response");
    assert_eq!(repaired.status(), StatusCode::OK);
    let repaired = json(repaired).await;
    assert_eq!(repaired["operation"]["id"], repaired_key.as_str());
    assert_eq!(repaired["operation"]["state"], "completed");
    let current = ctx
        .managed_repository()
        .inventory_status(&owner)
        .await
        .expect("inventory after repaired refresh");
    assert_eq!(repaired["result"]["generation"], current.generation);
    assert_eq!(repaired["result"]["entries"], current.entries);
    assert_eq!(
        repaired["result"]["last_error"],
        serde_json::to_value(&current.last_error).expect("inventory error state")
    );
    let response_observed = chrono::DateTime::parse_from_rfc3339(
        repaired["result"]["observed_at"]
            .as_str()
            .expect("completed observation timestamp"),
    )
    .expect("RFC3339 observation timestamp");
    assert_eq!(
        response_observed.timestamp_micros(),
        current
            .observed_at
            .expect("persisted observation timestamp")
            .timestamp_micros(),
        "the response and PostgreSQL row identify the same observation at database precision"
    );
    let completed = ctx
        .managed_repository()
        .api_operation(&owner, &repaired_key)
        .await
        .expect("completed repaired operation");
    assert_eq!(completed.state, "completed");
    assert_eq!(completed.result.as_ref(), Some(&repaired["result"]));

    drop(app);
    drop(ctx);
    drop(db);
    database.drop_now().await;
}
