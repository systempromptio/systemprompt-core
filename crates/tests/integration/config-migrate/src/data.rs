//! `config migrate --data` against a freshly installed schema: personal keys
//! move onto their credential id, every audit row gets its credential and
//! endpoint, the legacy map is recorded, host filters change tag, and a second
//! run changes nothing (credential.md §9.3).

use std::collections::BTreeMap;

use systemprompt_cli::commands::admin::config::migrate::data::apply_with;
use systemprompt_cli::commands::admin::config::migrate::legacy_map::{
    LegacyMap, LegacyProviderTarget,
};
use systemprompt_test_fixtures::DisposableDb;

fn target(provider: &str, endpoint: &str, credential: &str) -> LegacyProviderTarget {
    LegacyProviderTarget {
        provider: provider.to_owned(),
        endpoint: endpoint.to_owned(),
        credential: credential.to_owned(),
    }
}

fn map() -> LegacyMap {
    LegacyMap {
        providers: BTreeMap::from([
            (
                "anthropic".to_owned(),
                target("anthropic", "anthropic/api", "anthropic"),
            ),
            (
                "opencode-go-anthropic".to_owned(),
                target("opencode-go", "opencode-go/messages", "opencode-go"),
            ),
        ]),
        route_models: BTreeMap::new(),
        dropped_models: Vec::new(),
    }
}

async fn seed(pg: &sqlx::PgPool) {
    sqlx::query(
        "INSERT INTO users (id, name, email) VALUES ('u-mig', 'u-mig', 'u-mig@example.com')",
    )
    .execute(pg)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO user_provider_credentials (user_id, credential, ciphertext, nonce) \
         VALUES ('u-mig', 'opencode-go-anthropic', 'sealed', 'nonce')",
    )
    .execute(pg)
    .await
    .unwrap();
    for (id, provider, served) in [
        ("r1", "opencode-go-anthropic", None),
        ("r2", "opencode-go-anthropic", None),
        ("r3", "anthropic", None),
        ("r4", "anthropic", Some("opencode-go-anthropic")),
    ] {
        sqlx::query(
            "INSERT INTO ai_requests (id, request_id, user_id, context_id, actor_kind, actor_id, \
             provider, served_provider, model, status) \
             VALUES ($1, $1, 'u-mig', 'ctx', 'user', 'u-mig', $2, $3, 'm', 'completed')",
        )
        .bind(id)
        .bind(provider)
        .bind(served)
        .execute(pg)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO bridge_user_host_model_prefs (user_id, host_id, model_protocols) \
         VALUES ('u-mig', 'claude-code', ARRAY['anthropic', 'openai'])",
    )
    .execute(pg)
    .await
    .unwrap();
}

#[tokio::test]
async fn data_migration_moves_keys_backfills_audit_and_is_idempotent() {
    let db = DisposableDb::with_schema("cfgmig").await;
    let pool = db.test_pool().await;
    let pg = pool.write_pool();
    seed(&pg).await;

    let report = apply_with(&pool, &map())
        .await
        .expect("data migration runs");
    assert_eq!(report.legacy_entries, 2);
    assert_eq!(report.personal_keys_moved, 1);
    assert_eq!(report.audit_rows_backfilled, 4);
    assert_eq!(report.host_filters_rewritten, 1);

    let key: (String,) =
        sqlx::query_as("SELECT credential FROM user_provider_credentials WHERE user_id = 'u-mig'")
            .fetch_one(pg.as_ref())
            .await
            .unwrap();
    assert_eq!(key.0, "opencode-go");
    let rows: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, credential, endpoint FROM ai_requests WHERE user_id = 'u-mig' ORDER BY id",
    )
    .fetch_all(pg.as_ref())
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "r1".to_owned(),
                Some("opencode-go".to_owned()),
                Some("opencode-go/messages".to_owned())
            ),
            (
                "r2".to_owned(),
                Some("opencode-go".to_owned()),
                Some("opencode-go/messages".to_owned())
            ),
            (
                "r3".to_owned(),
                Some("anthropic".to_owned()),
                Some("anthropic/api".to_owned())
            ),
            (
                "r4".to_owned(),
                Some("opencode-go".to_owned()),
                Some("opencode-go/messages".to_owned())
            ),
        ]
    );
    let providers: Vec<(String,)> =
        sqlx::query_as("SELECT provider FROM ai_requests WHERE user_id = 'u-mig' ORDER BY id")
            .fetch_all(pg.as_ref())
            .await
            .unwrap();
    assert_eq!(
        providers[0].0, "opencode-go-anthropic",
        "provider keeps its historical value"
    );
    let mapped: (i64,) = sqlx::query_as("SELECT count(*) FROM ai_catalog_legacy_map")
        .fetch_one(pg.as_ref())
        .await
        .unwrap();
    assert_eq!(mapped.0, 2);
    let tags: (Vec<String>,) = sqlx::query_as(
        "SELECT model_protocols FROM bridge_user_host_model_prefs WHERE user_id = 'u-mig'",
    )
    .fetch_one(pg.as_ref())
    .await
    .unwrap();
    assert_eq!(tags.0, vec!["claude".to_owned(), "openai".to_owned()]);

    let again = apply_with(&pool, &map()).await.expect("second run");
    assert_eq!(again.personal_keys_moved, 0);
    assert_eq!(again.audit_rows_backfilled, 0);
    assert_eq!(again.host_filters_rewritten, 0);
    db.drop_now().await;
}

#[tokio::test]
async fn data_migration_moves_access_rules_from_legacy_routes_onto_model_entities() {
    let db = DisposableDb::with_schema("cfgmigroute").await;
    let pool = db.test_pool().await;
    let pg = pool.write_pool();
    for route in ["star-abc123", "catalog"] {
        sqlx::query(
            "INSERT INTO access_control_entities (entity_type, entity_id, default_included, \
             source) VALUES ('gateway_route', $1, false, 'code:services')",
        )
        .bind(route)
        .execute(pg.as_ref())
        .await
        .unwrap();
    }
    for (route, value) in [
        ("star-abc123", "eng"),
        ("star-abc123", "admin"),
        ("catalog", "admin"),
    ] {
        sqlx::query(
            "INSERT INTO access_control_rules (entity_type, entity_id, rule_type, rule_value, \
             access, source) VALUES ('gateway_route', $1, 'role', $2, 'allow', 'code:services')",
        )
        .bind(route)
        .bind(value)
        .execute(pg.as_ref())
        .await
        .unwrap();
    }
    let mut routed = map();
    routed
        .route_models
        .insert("star-abc123".to_owned(), "claude-*".to_owned());
    routed
        .route_models
        .insert("catalog".to_owned(), "claude-*".to_owned());
    routed
        .route_models
        .insert("absent".to_owned(), "other-*".to_owned());

    let report = apply_with(&pool, &routed).await.expect("data migration");

    assert_eq!(
        report.route_rules_moved_to_models, 2,
        "the duplicate admin grant is dropped"
    );
    let rules: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT entity_type, entity_id, rule_value FROM access_control_rules ORDER BY rule_value",
    )
    .fetch_all(pg.as_ref())
    .await
    .unwrap();
    assert_eq!(
        rules,
        vec![
            (
                "model".to_owned(),
                "claude-*".to_owned(),
                "admin".to_owned()
            ),
            ("model".to_owned(), "claude-*".to_owned(), "eng".to_owned())
        ]
    );
    let legacy: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM access_control_entities WHERE entity_type = 'gateway_route'",
    )
    .fetch_one(pg.as_ref())
    .await
    .unwrap();
    assert_eq!(legacy.0, 0);

    let again = apply_with(&pool, &routed).await.expect("second run");
    assert_eq!(again.route_rules_moved_to_models, 0);
    db.drop_now().await;
}
