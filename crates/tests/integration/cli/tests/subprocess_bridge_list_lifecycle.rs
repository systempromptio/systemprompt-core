use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;
use systemprompt_cli_integration_tests::full_bootstrap::{
    TEST_MANIFEST_SIGNING_SEED, TEST_OAUTH_AT_REST_PEPPER, isolated_fixture, systemprompt_bin,
};
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_oauth::repository::{BridgeSessionRepository, UpsertBridgeSession};
use systemprompt_test_fixtures::{DisposableDb, seed_user_row, seed_user_row_with_roles};

fn card_field<'a>(card: &'a Value, heading: &str) -> &'a Value {
    card["sections"]
        .as_array()
        .expect("bridge card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .map(|section| &section["content"])
        .unwrap_or_else(|| panic!("missing bridge card field {heading}: {card}"))
}

fn sanitized_child_output(output: &[u8], database_url: &str) -> String {
    [
        database_url,
        TEST_OAUTH_AT_REST_PEPPER,
        TEST_MANIFEST_SIGNING_SEED,
    ]
    .iter()
    .fold(
        String::from_utf8_lossy(output).into_owned(),
        |safe, secret| safe.replace(secret, "<REDACTED>"),
    )
}

fn bridge_json(database_url: &str, profile: &std::path::Path, args: &[&str]) -> Value {
    let mut command = Command::new(systemprompt_bin());
    command
        .env_remove("RUST_LOG")
        .env_remove("SYSTEMPROMPT_PROFILE")
        .env_remove("INTERNAL_DATABASE_URL")
        .env_remove("SYSTEMPROMPT_CLI_REMOTE")
        .env_remove("SYSTEMPROMPT_DEPLOYMENT_HOST")
        .env_remove("FLY_APP_NAME")
        .env("DATABASE_URL", database_url)
        .env("OAUTH_AT_REST_PEPPER", TEST_OAUTH_AT_REST_PEPPER)
        .env("MANIFEST_SIGNING_SECRET_SEED", TEST_MANIFEST_SIGNING_SEED)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .args(["--non-interactive", "--no-color", "--json", "--profile"])
        .arg(profile)
        .args(args)
        .timeout(Duration::from_secs(120));
    let output = command.output().expect("run bounded bridge list command");
    let stdout = sanitized_child_output(&output.stdout, database_url);
    let stderr = sanitized_child_output(&output.stderr, database_url);
    assert!(
        output.status.success(),
        "bridge list command failed; stdout={stdout}; stderr={stderr}"
    );
    serde_json::from_slice(&output.stdout).expect("bridge list emits one JSON artifact")
}

fn projected_row(row: &systemprompt_oauth::repository::BridgeSessionRow) -> Value {
    serde_json::json!({
        "session_id": row.session_id.as_str(),
        "user_id": row.user_id.as_str(),
        "hostname": row.hostname,
        "bridge_version": row.bridge_version,
        "os": row.os,
        "last_heartbeat_at": row.last_heartbeat_at.to_rfc3339(),
        "last_activity_at": row.last_activity_at.map(|value| value.to_rfc3339()),
        "forwarded_total": row.forwarded_total
    })
}

fn sorted_sessions(card: &Value) -> Vec<Value> {
    let mut sessions = card_field(card, "sessions")
        .as_array()
        .expect("bridge sessions")
        .clone();
    sessions.sort_by(|left, right| {
        left["session_id"]
            .as_str()
            .cmp(&right["session_id"].as_str())
    });
    sessions
}

fn bridge_row(
    user_id: &UserId,
    session: &str,
    hostname: &str,
    forwarded: i64,
) -> UpsertBridgeSession {
    UpsertBridgeSession {
        session_id: SessionId::new(session),
        user_id: user_id.clone(),
        bridge_version: "0.58.0-test".to_owned(),
        os: "linux-test".to_owned(),
        hostname: hostname.to_owned(),
        last_activity_at: Some(chrono::Utc::now()),
        forwarded_total: forwarded,
        tokens_in_total: 10,
        tokens_out_total: 20,
    }
}

#[tokio::test]
async fn bridge_list_filters_owner_and_activity_window_with_exact_session_projection() {
    let database = DisposableDb::installed("cli_bridge_list")
        .await
        .expect("install isolated bridge database");
    let pool = database.pool().await.expect("bridge database pool");
    let owner = UserId::new("user_bridge_owner");
    let other = UserId::new("user_bridge_other");
    let admin = UserId::new("user_bridge_admin");
    seed_user_row_with_roles(
        &pool,
        &admin,
        "bridge-admin@test.invalid",
        &["admin".to_owned()],
    )
    .await
    .expect("seed configured bridge administrator");
    let raw = pool.pool_arc().expect("raw bridge pool");
    sqlx::query("UPDATE users SET name = 'testadmin' WHERE id = $1")
        .bind(admin.as_str())
        .execute(raw.as_ref())
        .await
        .expect("bind bridge administrator to configured username");
    seed_user_row(&pool, &owner, "bridge-owner@test.invalid")
        .await
        .expect("seed bridge owner");
    seed_user_row(&pool, &other, "bridge-other@test.invalid")
        .await
        .expect("seed bridge control user");
    let repo = BridgeSessionRepository::new(&pool).expect("bridge repository");
    repo.upsert(bridge_row(&owner, "bridge_owned_active", "owned-host", 7))
        .await
        .expect("seed owned active bridge");
    repo.upsert(bridge_row(&other, "bridge_other_active", "other-host", 11))
        .await
        .expect("seed other active bridge");
    repo.upsert(bridge_row(&owner, "bridge_owned_stale", "stale-host", 13))
        .await
        .expect("seed owned stale bridge");
    sqlx::query("UPDATE bridge_sessions SET last_heartbeat_at = NOW() - INTERVAL '1 hour' WHERE session_id = 'bridge_owned_stale'")
        .execute(raw.as_ref())
        .await
        .expect("age owned stale bridge");
    let mut expected_active = repo
        .list_active(Duration::from_secs(120))
        .await
        .expect("read durable active bridge rows")
        .iter()
        .map(projected_row)
        .collect::<Vec<_>>();
    expected_active.sort_by(|left, right| {
        left["session_id"]
            .as_str()
            .cmp(&right["session_id"].as_str())
    });
    assert_eq!(expected_active.len(), 2);
    let expected_owned = expected_active
        .iter()
        .filter(|row| row["user_id"] == owner.as_str())
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(expected_owned.len(), 1);

    let fixture = isolated_fixture(8080);
    let web = fixture.services_dir.join("web");
    std::fs::create_dir_all(web.join("templates")).expect("create bridge web templates");
    std::fs::create_dir_all(web.join("assets")).expect("create bridge web assets");
    let web_config_path = web.join("config.yaml");
    let web_config = std::fs::read_to_string(&web_config_path).expect("read bridge web config");
    std::fs::write(
        &web_config_path,
        format!(
            "paths:\n  templates: {}\n  assets: {}\n{web_config}",
            web.join("templates").display(),
            web.join("assets").display()
        ),
    )
    .expect("complete bridge web paths");
    let all = bridge_json(
        database.url(),
        &fixture.profile_path,
        &["admin", "bridge", "list", "--within-secs", "120"],
    );
    assert_eq!(card_field(&all, "within_secs"), 120);
    assert_eq!(sorted_sessions(&all), expected_active);
    assert!(!all.to_string().contains("bridge_owned_stale"));

    let owned = bridge_json(
        database.url(),
        &fixture.profile_path,
        &[
            "admin",
            "bridge",
            "list",
            "--within-secs",
            "120",
            "--user-id",
            owner.as_str(),
        ],
    );
    assert_eq!(sorted_sessions(&owned), expected_owned);

    drop(raw);
    pool.write_pool_arc().expect("write pool").close().await;
    drop(pool);
    database.drop_now().await;
}
