use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;
use systemprompt_cli_integration_tests::full_bootstrap::systemprompt_bin;
use systemprompt_test_fixtures::DisposableDb;

fn command(database_url: &str, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(systemprompt_bin());
    command.env_remove("RUST_LOG");
    command.env_remove("SYSTEMPROMPT_PROFILE");
    command.env("SYSTEMPROMPT_PROFILE", "__nonexistent__");
    command.args([
        "--non-interactive",
        "--no-color",
        "--json",
        "--database-url",
        database_url,
    ]);
    command.args(args);
    command.timeout(Duration::from_secs(120));
    command.output().expect("run bounded CLI subprocess")
}

fn redact(text: &[u8], database_url: &str) -> String {
    let mut sanitized = String::from_utf8_lossy(text).replace(database_url, "<database-url>");
    if let Ok(parsed) = url::Url::parse(database_url)
        && let Some(password) = parsed.password().filter(|value| !value.is_empty())
    {
        sanitized = sanitized.replace(password, "<database-password>");
    }
    sanitized
}

fn json_success(database_url: &str, args: &[&str]) -> Value {
    let output = command(database_url, args);
    assert!(
        output.status.success(),
        "CLI command failed: stdout={} stderr={}",
        redact(&output.stdout, database_url),
        redact(&output.stderr, database_url),
    );
    let stdout = redact(&output.stdout, database_url);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout must be one JSON artifact: {error}\n{stdout}"))
}

fn card_field<'a>(card: &'a Value, heading: &str) -> &'a Value {
    card["sections"]
        .as_array()
        .expect("card sections")
        .iter()
        .find(|section| section["heading"] == heading)
        .map(|section| &section["content"])
        .unwrap_or_else(|| panic!("missing card field {heading}: {card}"))
}

async fn raw_pool(database: &DisposableDb) -> sqlx::PgPool {
    database
        .pool()
        .await
        .expect("open isolated database")
        .pool_arc()
        .expect("raw PostgreSQL pool")
        .as_ref()
        .clone()
}

#[tokio::test]
async fn request_show_preserves_complete_client_evidence() {
    let database = DisposableDb::installed("cli_request_client_evidence")
        .await
        .expect("install isolated request database");
    let pool = raw_pool(&database).await;
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let user = format!("evidence_user_{suffix}");
    let request = format!("evidence_request_{suffix}");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&user)
        .bind(format!("{user}@example.invalid"))
        .execute(&pool)
        .await
        .expect("seed request owner");
    sqlx::query(
        "INSERT INTO ai_requests \
         (id, request_id, user_id, context_id, provider, model, requested_model, actor_kind, actor_id, status, \
          client_kind, client_attestation, input_tokens, output_tokens, cost_microdollars, latency_ms) \
         VALUES ($1, $1, $2, '00000000-0000-0000-0000-00000000c0de', 'openai', 'gpt-fixture', 'gpt-requested', 'user', $2, 'completed', \
          'codex', 'host-token', 12, 7, 125000, 43)",
    )
    .bind(&request)
    .bind(&user)
    .execute(&pool)
    .await
    .expect("seed AI request");
    sqlx::query(
        "INSERT INTO ai_request_client_evidence \
         (ai_request_id, kind_source, attested_host, declared_client, native_marker, ua_product, \
          ua_version, sdk_lang, sdk_package_version, sdk_runtime, sdk_runtime_version, sdk_os, sdk_arch) \
         VALUES ($1, 'host-token', 'codex', 'fixture-client', 'codex-turn-metadata', 'fixture-agent', \
          '1.2.3', 'rust', '0.58.0', 'tokio', '1.0', 'linux', 'x86_64')",
    )
    .bind(&request)
    .execute(&pool)
    .await
    .expect("seed complete client evidence");

    let shown = json_success(
        database.url(),
        &["infra", "logs", "request", "show", &request],
    );
    assert_eq!(shown["artifact_type"], "presentation_card");
    assert_eq!(shown["title"], "AI Request Details");
    assert_eq!(
        card_field(&shown, "request_id").as_str(),
        Some(request.as_str())
    );
    assert_eq!(card_field(&shown, "client"), "codex (host-token)");
    let evidence = card_field(&shown, "client_evidence");
    assert_eq!(
        evidence,
        &serde_json::json!({
            "kind_source": "host-token",
            "attested_host": "codex",
            "declared_client": "fixture-client",
            "native_marker": "codex-turn-metadata",
            "ua_product": "fixture-agent",
            "ua_version": "1.2.3",
            "sdk_lang": "rust",
            "sdk_package_version": "0.58.0",
            "sdk_runtime": "tokio",
            "sdk_runtime_version": "1.0",
            "sdk_os": "linux",
            "sdk_arch": "x86_64"
        })
    );

    drop(pool);
    database.drop_now().await;
}

#[tokio::test]
async fn projection_rebuild_status_and_bounded_sync_track_durable_backlog() {
    let database = DisposableDb::installed("cli_reporting_projection")
        .await
        .expect("install isolated reporting database");
    let pool = raw_pool(&database).await;

    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let user = format!("projection_user_{suffix}");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(&user)
        .bind(format!("{user}@example.invalid"))
        .execute(&pool)
        .await
        .expect("seed projection owner");

    let rebuilt = json_success(database.url(), &["analytics", "projection", "rebuild"]);
    assert_eq!(rebuilt["artifact_type"], "presentation_card");
    assert_eq!(rebuilt["title"], "Reporting projection");
    assert_eq!(card_field(&rebuilt, "initialized"), true);
    let generation = card_field(&rebuilt, "generation")
        .as_i64()
        .expect("numeric projection generation");
    assert!(generation >= 1);
    let baseline = json_success(
        database.url(),
        &["analytics", "projection", "sync", "--limit", "200"],
    );
    assert_eq!(card_field(&baseline, "pending_count"), 0);

    for ordinal in 1..=2 {
        sqlx::query(
            "INSERT INTO logs (id, level, module, message, user_id, session_id, trace_id) \
             VALUES ($1, 'INFO', 'coverage.projection', 'pending projection fact', $2, $3, $4)",
        )
        .bind(format!("projection_log_{suffix}_{ordinal}"))
        .bind(&user)
        .bind(format!("projection_session_{suffix}_{ordinal}"))
        .bind(format!("projection_trace_{suffix}_{ordinal}"))
        .execute(&pool)
        .await
        .expect("seed pending reporting fact");
    }

    let pending = json_success(database.url(), &["analytics", "projection", "status"]);
    assert_eq!(card_field(&pending, "initialized"), true);
    assert_eq!(card_field(&pending, "generation"), generation);
    assert_eq!(card_field(&pending, "pending_count"), 2);
    assert!(card_field(&pending, "oldest_pending_at").is_string());

    let synced = json_success(
        database.url(),
        &["analytics", "projection", "sync", "--limit", "1"],
    );
    assert_eq!(card_field(&synced, "pending_count"), 1);
    assert!(card_field(&synced, "last_processed_at").is_string());
    let processed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_outbox \
         WHERE consumer = 'analytics_reporting' AND processed_at IS NOT NULL \
           AND fact #>> '{data,source}' = 'logs' \
           AND fact #>> '{data,key}' LIKE $1",
    )
    .bind(format!("projection_log_{suffix}_%"))
    .fetch_one(&pool)
    .await
    .expect("count durably acknowledged reporting facts");
    assert_eq!(processed, 1);

    let completed = json_success(
        database.url(),
        &["analytics", "projection", "sync", "--limit", "1"],
    );
    assert_eq!(card_field(&completed, "pending_count"), 0);
    let processed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM event_outbox \
         WHERE consumer = 'analytics_reporting' AND processed_at IS NOT NULL \
           AND fact #>> '{data,source}' = 'logs' \
           AND fact #>> '{data,key}' LIKE $1",
    )
    .bind(format!("projection_log_{suffix}_%"))
    .fetch_one(&pool)
    .await
    .expect("count all durably acknowledged reporting facts");
    assert_eq!(processed, 2);

    drop(pool);
    database.drop_now().await;
}
