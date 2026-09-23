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
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        let stdout = redact(&output.stdout, database_url);
        panic!("stdout must be one JSON artifact: {error}\n{stdout}")
    })
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
