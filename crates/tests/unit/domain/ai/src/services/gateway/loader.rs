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

async fn pool() -> Option<DbPool> {
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
    let Some(pool) = pool().await else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let report = load_gateway_policies_from_yaml(&pool, dir.path())
        .await
        .expect("missing file is ok");
    assert_eq!(report.inserted, 0);
    assert_eq!(report.updated, 0);
    assert_eq!(report.deleted, 0);
    assert_eq!(report.skipped, 0);
}

#[tokio::test]
async fn malformed_yaml_is_rejected_with_invalid_data() {
    let Some(pool) = pool().await else {
        return;
    };
    let dir = tempfile::tempdir().expect("tempdir");
    let gateway_dir = dir.path().join("gateway");
    std::fs::create_dir_all(&gateway_dir).expect("mkdir");
    std::fs::write(gateway_dir.join("policies.yaml"), "policies: [").expect("write");
    let err = load_gateway_policies_from_yaml(&pool, dir.path())
        .await
        .expect_err("must fail");
    assert!(err.to_string().contains("policies.yaml"));
}

#[tokio::test]
async fn unknown_yaml_fields_are_rejected() {
    let Some(pool) = pool().await else {
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
    let err = load_gateway_policies_from_yaml(&pool, dir.path())
        .await
        .expect_err("deny_unknown_fields must reject");
    assert!(err.to_string().contains("extra_field"));
}

#[tokio::test]
async fn ingest_inserts_then_skips_without_override() {
    let Some(pool) = pool().await else {
        return;
    };
    let name = unique_name("ingest-skip");
    let yaml = config_yaml(&[&name]);
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&yaml).expect("parse");
    let service = GatewayPolicyIngestionService::new(&pool).expect("service");

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
    let Some(pool) = pool().await else {
        return;
    };
    let name = unique_name("ingest-override");
    let service = GatewayPolicyIngestionService::new(&pool).expect("service");
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
    let Some(pool) = pool().await else {
        return;
    };
    let name = unique_name("ingest-disabled");
    let yaml = format!("policies:\n  - name: {name}\n    enabled: false\n");
    let cfg: GatewayPolicyConfig = serde_yaml::from_str(&yaml).expect("parse");
    let service = GatewayPolicyIngestionService::new(&pool).expect("service");
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
    let Some(pool) = pool().await else {
        return;
    };
    let cfg: GatewayPolicyConfig =
        serde_yaml::from_str("policies:\n  - name: '  '\n").expect("parse");
    let service = GatewayPolicyIngestionService::new(&pool).expect("service");
    let err = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect_err("empty name rejected");
    assert!(err.to_string().contains("policies[0].name"));
}

#[tokio::test]
async fn duplicate_policy_names_fail_validation() {
    let Some(pool) = pool().await else {
        return;
    };
    let name = unique_name("dup");
    let cfg: GatewayPolicyConfig =
        serde_yaml::from_str(&config_yaml(&[&name, &name])).expect("parse");
    let service = GatewayPolicyIngestionService::new(&pool).expect("service");
    let err = service
        .ingest_config(&cfg, GatewayPolicyIngestOptions::default())
        .await
        .expect_err("duplicate rejected");
    assert!(err.to_string().contains("duplicate policy name"));
}
