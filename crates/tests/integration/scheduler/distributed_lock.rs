//! Regression test for the scheduler's cross-replica single-execution
//! guarantee.
//!
//! When two scheduler replicas share one Postgres database and both have the
//! same job scheduled on the same cron, only ONE of them may actually run the
//! job per tick. The in-process `RunningJobs` set cannot enforce this — each
//! `SchedulerService` owns its own set — so the guarantee rests entirely on
//! the Postgres advisory lock claimed by `dispatch::execute_job` when
//! `SchedulerConfig.distributed_lock` is true.
//!
//! These tests encode that guarantee:
//!
//! * `distributed_lock_runs_job_once_across_replicas` asserts that with the
//!   lock enabled, `scheduled_jobs.run_count` advances roughly once per elapsed
//!   cron tick — not twice — proving the duplicate execution is suppressed.
//! * `without_distributed_lock_job_double_fires` is the negative control: it
//!   removes the lock and shows the run count roughly doubles, documenting
//!   exactly what the lock fixes.
//!
//! Both tests are DB-backed: they need a reachable Postgres (`DATABASE_URL`).

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use systemprompt_analytics::AnalyticsService;
use systemprompt_database::{Database, DbPool};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::profile::{ContentNegotiationConfig, PathsConfig, SecurityHeadersConfig};
use systemprompt_models::{AppPaths, Config, RouteClassifier};
use systemprompt_mcp::services::registry::RegistryService;
use systemprompt_runtime::{AppContext, AppContextParts, ModuleApiRegistry};
use systemprompt_scheduler::{JobConfig, SchedulerConfig, SchedulerService};
use systemprompt_security::authz::{DenyAllHook, NullAuditSink};
use systemprompt_traits::{Job, JobContext, JobResult, ProviderResult};
use systemprompt_test_fixtures::{fixture_system_admin, fixture_user_id};

/// Job name used by both replicas in this test. Unique enough not to collide
/// with the built-in jobs discovered via `inventory`.
const TEST_JOB_NAME: &str = "test_distributed_lock_probe";

/// A trivial job whose only purpose is to be scheduled. The scheduler
/// increments `scheduled_jobs.run_count` for every execution it dispatches,
/// so the job body itself does not need to touch the database — the run
/// count is the observable signal.
#[derive(Debug, Clone, Copy)]
struct DistributedLockProbeJob;

#[async_trait]
impl Job for DistributedLockProbeJob {
    fn name(&self) -> &'static str {
        TEST_JOB_NAME
    }

    fn description(&self) -> &'static str {
        "Test-only job exercising the scheduler distributed advisory lock"
    }

    fn schedule(&self) -> &'static str {
        // Every second — overridden per test via JobConfig anyway.
        "* * * * * *"
    }

    async fn execute(&self, _ctx: &JobContext) -> ProviderResult<JobResult> {
        Ok(JobResult::success())
    }
}

systemprompt_traits::submit_job!(&DistributedLockProbeJob);

/// Resolve the integration-test database URL from the environment.
fn test_database_url() -> Result<String> {
    dotenvy::dotenv().ok();
    std::env::var("DATABASE_URL")
        .map_err(|_e| anyhow::anyhow!("DATABASE_URL must be set for this DB-backed test"))
}

/// Build a `DbPool` against the integration-test database.
async fn test_db_pool(url: &str) -> Result<DbPool> {
    let database = Database::new_postgres(url).await?;
    Ok(Arc::new(database))
}

/// Minimal `Config` for the test. The scheduler never reads these fields —
/// jobs receive the context type-erased via `JobContext` and the probe job
/// ignores it — but `AppContext` requires a concrete `Config`.
fn test_config(database_url: &str) -> Config {
    Config {
        instance_id: "scheduler-distributed-lock-test".to_string(),
        max_concurrent_streams: 16,
        sitename: "test".to_string(),
        database_type: "postgres".to_string(),
        database_url: database_url.to_string(),
        database_write_url: None,
        github_link: String::new(),
        github_token: None,
        system_path: "/tmp".to_string(),
        services_path: "/tmp".to_string(),
        bin_path: "/tmp".to_string(),
        skills_path: "/tmp".to_string(),
        settings_path: "/tmp".to_string(),
        content_config_path: "/tmp".to_string(),
        geoip_database_path: None,
        web_path: "/tmp".to_string(),
        web_config_path: "/tmp".to_string(),
        web_metadata_path: "/tmp".to_string(),
        host: "127.0.0.1".to_string(),
        port: 0,
        api_server_url: "http://127.0.0.1".to_string(),
        api_internal_url: "http://127.0.0.1".to_string(),
        api_external_url: "http://127.0.0.1".to_string(),
        jwt_issuer: "test".to_string(),
        jwt_access_token_expiration: 3600,
        jwt_refresh_token_expiration: 86_400,
        jwt_audiences: vec![],
        allowed_resource_audiences: vec![],
        trusted_issuers: vec![],
        signing_key_path: std::path::PathBuf::from("signing_key.pem"),
        use_https: false,
        rate_limits: RateLimitConfig::default(),
        cors_allowed_origins: vec![],
        trusted_proxies: vec![],
        is_cloud: false,
        system_admin_username: "admin".to_string(),
        content_negotiation: ContentNegotiationConfig::default(),
        security_headers: SecurityHeadersConfig::default(),
        allow_registration: false,
    }
}

/// Build a minimal `AppContext` for the test.
///
/// `SchedulerService::new` requires a real `Arc<AppContext>`, but the
/// scheduler only ever forwards the context's `app_paths` into the
/// type-erased `JobContext`, and the probe job ignores it entirely. We
/// therefore assemble the lightest viable context via `AppContext::from_parts`
/// rather than going through the full profile/config bootstrap — keeping the
/// test dependent only on `DATABASE_URL`.
fn test_app_context(pool: &DbPool, database_url: &str) -> Result<Arc<AppContext>> {
    let paths = PathsConfig {
        system: "/tmp".to_string(),
        services: "/tmp".to_string(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    let app_paths = Arc::new(AppPaths::from_profile(&paths)?);

    let parts = AppContextParts {
        config: Arc::new(test_config(database_url)),
        database: Arc::clone(pool),
        api_registry: Arc::new(ModuleApiRegistry::new()),
        extension_registry: Arc::new(ExtensionRegistry::new()),
        geoip_reader: None,
        content_config: None,
        route_classifier: Arc::new(RouteClassifier::new(None)),
        analytics_service: Arc::new(AnalyticsService::new(pool, None, None)?),
        fingerprint_repo: None,
        user_service: None,
        app_paths,
        marketplace_filter: Arc::new(AllowAllFilter),
        event_bridge: Arc::new(OnceLock::new()),
        system_admin: Arc::new(fixture_system_admin("admin")),
        mcp_registry: RegistryService::new(fixture_user_id()),
        authz_hook: Arc::new(DenyAllHook::new(Arc::new(NullAuditSink))),
    };

    Ok(Arc::new(AppContext::from_parts(parts)))
}

/// Scheduler config with exactly the probe job, on a one-second cron.
fn probe_config(distributed_lock: bool) -> SchedulerConfig {
    SchedulerConfig {
        enabled: true,
        jobs: vec![
            JobConfig::new(TEST_JOB_NAME, fixture_user_id())
                .with_schedule("* * * * * *"),
        ],
        bootstrap_jobs: vec![],
        distributed_lock,
    }
}

/// Reset the probe job's `scheduled_jobs` row so each test starts from a
/// known baseline regardless of prior runs.
async fn reset_job_row(pool: &DbPool) -> Result<()> {
    sqlx::query("DELETE FROM scheduled_jobs WHERE job_name = $1")
        .bind(TEST_JOB_NAME)
        .execute(pool.write_pool_arc()?.as_ref())
        .await?;
    Ok(())
}

/// Read the current `run_count` for the probe job, or 0 if the row is absent.
async fn read_run_count(pool: &DbPool) -> Result<i32> {
    let count: Option<i32> =
        sqlx::query_scalar("SELECT run_count FROM scheduled_jobs WHERE job_name = $1")
            .bind(TEST_JOB_NAME)
            .fetch_optional(pool.read_pool_arc()?.as_ref())
            .await?;
    Ok(count.unwrap_or(0))
}

/// Run two scheduler replicas against one database for `window`, then return
/// the observed `run_count` for the probe job.
async fn run_two_replicas(distributed_lock: bool, window: Duration) -> Result<i32> {
    let database_url = test_database_url()?;
    let pool = test_db_pool(&database_url).await?;
    reset_job_row(&pool).await?;

    let app_context = test_app_context(&pool, &database_url)?;

    // Two independent SchedulerService instances — two "replicas" — sharing
    // one database, exactly as two processes behind a load balancer would.
    let replica_a = SchedulerService::new(
        probe_config(distributed_lock),
        Arc::clone(&pool),
        Arc::clone(&app_context),
    )?;
    let replica_b = SchedulerService::new(
        probe_config(distributed_lock),
        Arc::clone(&pool),
        Arc::clone(&app_context),
    )?;

    replica_a.start().await?;
    replica_b.start().await?;

    tokio::time::sleep(window).await;

    read_run_count(&pool).await
}

/// Regression test for the cross-replica single-execution guarantee.
///
/// Both phases share the one `scheduled_jobs` probe row, so they must run
/// sequentially in a single test — two `#[tokio::test]` functions would race
/// on that row under the default parallel test harness.
///
/// Phase 1 (lock enabled): two replicas sharing a database run the job ONCE
/// per tick. Over a `W`-second window we expect roughly `W` one-second ticks;
/// cron alignment puts the true count in `W - 1 ..= W + 1`. The defining
/// property of the fix is that the count stays near the single-replica rate
/// and does not approach `2 * W`.
///
/// Phase 2 (negative control, lock disabled): the same setup double-fires —
/// both replicas dispatch every tick — documenting exactly what the lock
/// prevents.
#[tokio::test]
async fn distributed_lock_suppresses_duplicate_execution() -> Result<()> {
    let window_secs: u64 = 6;
    let window = Duration::from_secs(window_secs);
    let single_replica_ceiling = window_secs as i32 + 2;

    let locked = run_two_replicas(true, window).await?;
    assert!(
        locked >= 3,
        "expected the job to fire several times in {window_secs}s, got run_count={locked}"
    );
    assert!(
        locked <= single_replica_ceiling,
        "distributed lock failed to suppress duplicate execution: run_count={locked} exceeds \
         the single-replica ceiling of {single_replica_ceiling}"
    );

    let unlocked = run_two_replicas(false, window).await?;
    assert!(
        unlocked > single_replica_ceiling,
        "expected double-firing without the lock: run_count={unlocked} should exceed the \
         single-replica ceiling of {single_replica_ceiling}"
    );

    assert!(
        locked < unlocked,
        "the lock must yield meaningfully fewer runs than the unlocked control: \
         locked={locked}, unlocked={unlocked}"
    );

    Ok(())
}
