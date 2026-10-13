//! The two refusals in the MCP startup gate.
//!
//! `handle_missing_servers` fires when the orchestrator brought up fewer
//! servers than were required; `verify_database_registration` fires when it
//! brought them all up but the database does not hold a `running` row for
//! each — the gap between those two claims is the race the second check
//! exists to catch. Both must fail startup, and both must name every server
//! that failed and why, because that message is the whole diagnosis an
//! operator gets from a refused boot.

use systemprompt_api::services::server::lifecycle::reconciliation::{
    VERIFY_ATTEMPTS, VERIFY_BACKOFF, handle_missing_servers, verify_database_registration,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{InstanceId, UserId};
use systemprompt_models::auth::JwtAudience;
use systemprompt_models::mcp::deployment::OAuthRequirement;
use systemprompt_models::mcp::{McpServerConfig, McpServerType};
use systemprompt_test_fixtures::{
    closed_db_pool, ensure_test_bootstrap, test_app_context, test_db_pool,
};

fn orchestrator(
    ctx: &systemprompt_runtime::AppContext,
) -> std::sync::Arc<systemprompt_mcp::services::McpOrchestrator> {
    std::sync::Arc::new(
        systemprompt_mcp::services::McpOrchestrator::new(
            (**ctx.service_repository()).clone(),
            std::sync::Arc::clone(ctx.app_paths_arc()),
            ctx.mcp_registry().clone(),
        )
        .expect("orchestrator"),
    )
}

fn unique_name(prefix: &str) -> String {
    format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    )
}

fn required(name: &str) -> McpServerConfig {
    McpServerConfig {
        name: name.to_owned(),
        owner: UserId::new("fixture"),
        server_type: McpServerType::Internal,
        binary: Some(format!("{name}-bin")),
        enabled: true,
        display_in_web: false,
        port: Some(0),
        crate_path: std::path::PathBuf::from("."),
        display_name: name.to_owned(),
        description: String::new(),
        capabilities: vec![],
        schemas: vec![],
        oauth: OAuthRequirement {
            required: false,
            scopes: vec![],
            audience: JwtAudience::Mcp,
            client_id: None,
            ema: false,
        },
        tools: std::collections::HashMap::new(),
        model_config: None,
        env_vars: vec![],
        version: "0.0.1".to_owned(),
        host: "127.0.0.1".to_owned(),
        module_name: "mcp".to_owned(),
        protocol: "mcp".to_owned(),
        remote_endpoint: String::new(),
        external_auth: None,
        headers: std::collections::HashMap::new(),
    }
}

async fn live_pool() -> DbPool {
    ensure_test_bootstrap();
    test_db_pool().await
}

async fn seed(pool: &DbPool, instance: &InstanceId, name: &str, status: &str) {
    let inner = pool.pool();
    sqlx::query(
        "INSERT INTO services (instance_id, name, module_name, status, port, pid)
         VALUES ($4, $1, 'mcp', $2, 0, $3)
         ON CONFLICT (instance_id, name) DO UPDATE SET status = $2",
    )
    .bind(name)
    .bind(status)
    .bind(i32::try_from(std::process::id()).expect("pid fits in i32"))
    .bind(instance.as_str())
    .execute(inner.as_ref())
    .await
    .expect("seed the services row");
}

async fn drop_row(pool: &DbPool, name: &str) {
    let inner = pool.pool();
    sqlx::query("DELETE FROM services WHERE name = $1")
        .bind(name)
        .execute(inner.as_ref())
        .await
        .expect("clean up the services row");
}

#[tokio::test]
async fn a_required_server_with_a_running_row_passes_verification() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("verified");
    seed(&pool, &ctx.config().instance_id, &name, "running").await;

    let outcome = verify_database_registration(&[required(&name)], &ctx, None).await;

    assert!(
        outcome.is_ok(),
        "a running row is exactly what this check is looking for: {outcome:?}"
    );

    drop_row(&pool, &name).await;
}

#[tokio::test]
async fn a_required_server_with_no_row_at_all_fails_startup_and_is_named() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("unregistered");

    let error = verify_database_registration(&[required(&name)], &ctx, None)
        .await
        .map(|_| ())
        .expect_err("an unregistered required server must not be allowed through");

    let message = error.to_string();
    assert!(
        message.contains(&name) && message.contains("not in database"),
        "the failure must name the server and why it failed; got: {message}"
    );
}

#[tokio::test]
async fn a_required_server_registered_in_a_non_running_status_fails_and_reports_that_status() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("halfup");
    seed(&pool, &ctx.config().instance_id, &name, "starting").await;

    let error = verify_database_registration(&[required(&name)], &ctx, None)
        .await
        .map(|_| ())
        .expect_err("a row that is not `running` is not a started server");

    let message = error.to_string();
    assert!(
        message.contains(&name) && message.contains("status: starting"),
        "the operator needs the status the row actually held; got: {message}"
    );

    drop_row(&pool, &name).await;
}

#[tokio::test]
async fn every_failing_server_is_reported_not_just_the_first() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let missing = unique_name("missing");
    let stopped = unique_name("stopped");
    seed(&pool, &ctx.config().instance_id, &stopped, "stopped").await;

    let error = verify_database_registration(&[required(&missing), required(&stopped)], &ctx, None)
        .await
        .map(|_| ())
        .expect_err("two failing servers is still a failure");

    let message = error.to_string();
    assert!(
        message.contains(&missing) && message.contains(&stopped),
        "a boot refused for several reasons must list all of them; got: {message}"
    );

    drop_row(&pool, &stopped).await;
}

#[tokio::test]
async fn an_unreachable_database_fails_verification_rather_than_passing_it() {
    let boot = ensure_test_bootstrap();
    let pool = closed_db_pool().await;
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("unreachable");

    let error = verify_database_registration(&[required(&name)], &ctx, None)
        .await
        .map(|_| ())
        .expect_err("a database that cannot be read cannot confirm anything");

    let message = error.to_string();
    assert!(
        message.contains(&name) && message.contains("db error"),
        "a lookup failure must be distinguished from a missing row; got: {message}"
    );
}

// Why: this is the other refusal in the same startup gate, and it fires
// earlier — the orchestrator reported fewer servers running than were
// required. What the boot log has to carry is *which* ones, because the
// message is also the build instruction the operator follows.
#[tokio::test]
async fn a_required_server_that_never_started_is_named_in_the_refusal() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("neverstarted");

    let error = handle_missing_servers(&[required(&name)], &orchestrator(&ctx), None)
        .await
        .map(|_| ())
        .expect_err("a required server that is not running must fail startup");

    let message = error.to_string();
    assert!(
        message.contains(&name),
        "the operator has to be told which server to build; got: {message}"
    );
    assert!(
        message.contains("--bin"),
        "the refusal doubles as the build command for the missing binaries; got: {message}"
    );
}

#[tokio::test]
async fn several_servers_that_never_started_are_all_named() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let first = unique_name("absent_one");
    let second = unique_name("absent_two");

    let error = handle_missing_servers(
        &[required(&first), required(&second)],
        &orchestrator(&ctx),
        None,
    )
    .await
    .map(|_| ())
    .expect_err("two missing servers is still a failure");

    let message = error.to_string();
    assert!(
        message.contains(&first) && message.contains(&second),
        "a partial list would send the operator round the loop twice; got: {message}"
    );
    assert!(
        message.contains("2 required MCP server(s)"),
        "the count must match the list; got: {message}"
    );
}

#[tokio::test]
async fn a_row_that_turns_running_during_the_retry_window_passes_verification() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("late");
    seed(&pool, &ctx.config().instance_id, &name, "starting").await;

    let flipper = {
        let pool = pool.clone();
        let instance = ctx.config().instance_id.clone();
        let name = name.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            seed(&pool, &instance, &name, "running").await;
        })
    };

    let outcome = verify_database_registration(&[required(&name)], &ctx, None).await;
    flipper.await.expect("flip task");

    assert!(
        outcome.is_ok(),
        "a concurrent reconcile that finishes inside the bounded retry must pass: {outcome:?}"
    );

    drop_row(&pool, &name).await;
}

#[tokio::test]
async fn a_row_that_never_turns_running_fails_after_every_attempt() {
    let pool = live_pool().await;
    let boot = ensure_test_bootstrap();
    let ctx = test_app_context(&pool, &boot.database_url);
    let name = unique_name("stuck");
    seed(&pool, &ctx.config().instance_id, &name, "stopped").await;

    let started = std::time::Instant::now();
    let error = verify_database_registration(&[required(&name)], &ctx, None)
        .await
        .map(|_| ())
        .expect_err("a row that stays stopped must fail");
    let elapsed = started.elapsed();

    let expected_wait = VERIFY_BACKOFF * (VERIFY_ATTEMPTS - 1);
    assert!(
        elapsed >= expected_wait,
        "verification must retry {VERIFY_ATTEMPTS} times before refusing; took {elapsed:?}"
    );
    let message = error.to_string();
    assert!(
        message.contains(&format!("after {VERIFY_ATTEMPTS} attempts")),
        "the refusal reports how many attempts were made; got: {message}"
    );

    drop_row(&pool, &name).await;
}
