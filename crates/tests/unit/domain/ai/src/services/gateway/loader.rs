// Gateway-policy bootstrap: YAML file loading and DB ingestion semantics.
// The delete_orphans reconcile arm is exercised only through config validation
// here — a DB-level orphan sweep would race sibling tests sharing the table.

use serde_json::json;
use systemprompt_ai::{
    GatewayPolicyConfig, GatewayPolicyIngestOptions, GatewayPolicyIngestionService,
    load_gateway_policies_from_yaml,
};
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn pool_or_skip() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    ensure_test_bootstrap();
    Some(fixture_db_pool(&url).await.expect("pool"))
}

fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4())
}

fn config_yaml(names: &[&str]) -> String {
    let mut out = String::from("policies:\n");
    for name in names {
        out.push_str(&format!(
            "  - name: {name}\n    enabled: true\n    spec:\n      quota_windows:\n        - \
             window_seconds: 60\n          max_requests: 5\n"
        ));
    }
    out
}

#[tokio::test]
async fn missing_policies_file_is_a_noop() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let report = load_gateway_policies_from_yaml(
        &systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
        dir.path(),
    )
    .await
    .expect("missing file is ok");
    assert_eq!(report.inserted, 0);
    assert_eq!(report.updated, 0);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped, 0);
}

#[tokio::test]
async fn malformed_yaml_is_rejected_with_invalid_data() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let gateway_dir = dir.path().join("gateway");
    std::fs::create_dir_all(&gateway_dir).expect("mkdir");
    std::fs::write(gateway_dir.join("policies.yaml"), "policies: [").expect("write");
    let err = load_gateway_policies_from_yaml(
        &systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
        dir.path(),
    )
    .await
    .expect_err("must fail");
    assert!(err.to_string().contains("policies.yaml"));
}

#[tokio::test]
async fn unknown_yaml_fields_are_rejected() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let gateway_dir = dir.path().join("gateway");
    std::fs::create_dir_all(&gateway_dir).expect("mkdir");
    std::fs::write(
        gateway_dir.join("policies.yaml"),
        "policies: []\nextra_field: true\n",
    )
    .expect("write");
    let err = load_gateway_policies_from_yaml(
        &systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
        dir.path(),
    )
    .await
    .expect_err("deny_unknown_fields must reject");
    assert!(err.to_string().contains("extra_field"));
}

#[tokio::test]
async fn ingest_inserts_then_skips_without_override() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let name = unique_name("ingest-skip");
    let yaml = config_yaml(&[&name]);
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&yaml).expect("parse");
    let service = GatewayPolicyIngestionService::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
    );

    let first = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect("first ingest");
    assert_eq!(first.inserted, 1);
    assert_eq!(first.skipped, 0);

    let second = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect("second ingest");
    assert_eq!(second.inserted, 0);
    assert_eq!(second.skipped, 1);
    assert_eq!(second.updated, 0);
}

#[tokio::test]
async fn ingest_with_override_updates_existing_spec() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let name = unique_name("ingest-override");
    let service = GatewayPolicyIngestionService::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
    );
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&config_yaml(&[&name])).expect("parse");
    service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect("seed");

    let updated_yaml = format!(
        "policies:\n  - name: {name}\n    enabled: true\n    spec:\n      quota_windows:\n        \
         - window_seconds: 60\n          max_requests: 999\n"
    );
    let cfg2: GatewayPolicyConfig = serde_yaml::from_str(&updated_yaml).expect("parse");
    let report = service
        .ingest_config(
            &cfg2,
            GatewayPolicyIngestOptions {
                override_existing: true,
                delete_orphans: false,
            },
        )
        .await
        .expect("override ingest");
    assert_eq!(report.updated, 1);
    assert_eq!(report.inserted, 0);

    let repo = systemprompt_ai::AiGatewayPolicyRepository::new(&pool).expect("repo");
    let row = repo
        .list_for_global()
        .await
        .expect("list")
        .into_iter()
        .find(|r| r.name == name)
        .expect("row present");
    assert_eq!(
        row.spec.pointer("/quota_windows/0/max_requests").cloned(),
        Some(json!(999))
    );
}

#[tokio::test]
async fn disabled_policy_is_upserted_but_not_served() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let name = unique_name("ingest-disabled");
    let yaml = format!("policies:\n  - name: {name}\n    enabled: false\n");
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&yaml).expect("parse");
    let service = GatewayPolicyIngestionService::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
    );
    let report = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect("ingest");
    assert_eq!(report.inserted, 1);

    let repo = systemprompt_ai::AiGatewayPolicyRepository::new(&pool).expect("repo");
    let served = repo.list_for_global().await.expect("list");
    assert!(!served.iter().any(|r| r.name == name));
}

#[tokio::test]
async fn empty_policy_name_fails_validation() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let cfg: GatewayPolicyConfig =
        serde_yaml::from_str("policies:\n  - name: '  '\n").expect("parse");
    let service = GatewayPolicyIngestionService::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
    );
    let err = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect_err("empty name rejected");
    assert!(err.to_string().contains("policies[0].name"));
}

#[tokio::test]
async fn duplicate_policy_names_fail_validation() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let name = unique_name("dup");
    let cfg: GatewayPolicyConfig =
        serde_yaml::from_str(&config_yaml(&[&name, &name])).expect("parse");
    let service = GatewayPolicyIngestionService::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
    );
    let err = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect_err("duplicate rejected");
    assert!(err.to_string().contains("duplicate policy name"));
}

// NOTE FOR THE nextest CONFIG OWNER: the two tests below drive
// `load_gateway_policies_from_yaml`, which always ingests with
// `delete_orphans: true` against the *global* scope — it therefore deletes
// every `ai_gateway_policies` row not named by its YAML, including rows seeded
// by sibling tests in this file. They must be serialised against the rest of
// the gateway-policy suite (a `gateway-policy-db` test-group).

#[tokio::test]
async fn a_valid_policies_file_is_ingested_and_reconciles_the_table_to_it() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let name = unique_name("loader-happy");
    let orphan = unique_name("loader-orphan");
    let service = GatewayPolicyIngestionService::from_repository(
        systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
    );
    service
        .ingest_config(
            &serde_yaml::from_str(&config_yaml(&[&orphan])).expect("parse"),
            GatewayPolicyIngestOptions::default(),
        )
        .await
        .expect("seed an orphan the loader's file will not mention");

    let dir = tempfile::tempdir().expect("tempdir");
    let gateway_dir = dir.path().join("gateway");
    std::fs::create_dir_all(&gateway_dir).expect("mkdir");
    std::fs::write(gateway_dir.join("policies.yaml"), config_yaml(&[&name])).expect("write");

    let report = load_gateway_policies_from_yaml(
        &systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
        dir.path(),
    )
    .await
    .expect("valid file ingests");
    assert_eq!(report.inserted, 1, "the file's one policy must be inserted");

    let repo = systemprompt_ai::AiGatewayPolicyRepository::new(&pool).expect("repo");
    let served = repo.list_for_global().await.expect("list");
    assert_eq!(
        served.iter().filter(|r| r.name == name).count(),
        1,
        "the loaded policy must be served"
    );
    assert!(
        !served.iter().any(|r| r.name == orphan),
        "a policy absent from the file must be swept — the loader reconciles, it does not merge"
    );
    assert!(report.deleted >= 1, "the orphan sweep must be reported");

    // A second boot over the same file is idempotent, not a re-insert.
    let again = load_gateway_policies_from_yaml(
        &systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
        dir.path(),
    )
    .await
    .expect("second boot");
    assert_eq!(again.inserted, 0);
    assert_eq!(again.updated + again.skipped, 1);
}

#[tokio::test]
async fn an_unreadable_policies_path_is_an_error_rather_than_a_silent_noop() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    // A directory where the file is expected: readable metadata, but
    // `read_to_string` fails with something other than NotFound, so the loader
    // must not mistake it for "no config".
    std::fs::create_dir_all(dir.path().join("gateway").join("policies.yaml")).expect("mkdir");

    let err = load_gateway_policies_from_yaml(
        &systemprompt_ai::repository::AiGatewayPolicyRepository::new(&pool).expect("repository"),
        dir.path(),
    )
    .await
    .expect_err("an unreadable path must not be treated as an absent config");
    assert!(
        err.to_string().contains("gateway/policies.yaml"),
        "the error must name the file it failed to read, got {err}"
    );
}

#[tokio::test]
async fn a_service_built_from_a_repository_ingests_the_same_as_one_built_from_a_pool() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let name = unique_name("from-repo");
    let repo = systemprompt_ai::AiGatewayPolicyRepository::new(&pool).expect("repo");
    let service = GatewayPolicyIngestionService::from_repository(repo);

    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&config_yaml(&[&name])).expect("parse");
    let report = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect("ingest through the repository-built service");
    assert_eq!(report.inserted, 1);

    let served = systemprompt_ai::AiGatewayPolicyRepository::new(&pool)
        .expect("repo")
        .list_for_global()
        .await
        .expect("list");
    assert!(
        served.iter().any(|r| r.name == name),
        "the repository-built service must write through the same table"
    );
}
