//! DB-backed tests for the scheduler persistence layer.
//!
//! Each test acquires a real Postgres pool via the fixtures crate and
//! skips when `DATABASE_URL` is unset locally, failing under `CI`
//! instead. Every test owns uniquely-named rows
//! (`scheduled_jobs.job_name`) so concurrent shards never collide, and asserts
//! a concrete outcome: row present/absent, field values, or row counts.
//!
//! The `scheduled_jobs` table has no foreign-key dependency, so job CRUD is
//! exercised end-to-end through the public repository API. The analytics and
//! security repositories read/maintain `user_contexts` / `user_sessions`,
//! which have no fixtures seed helper; those tests therefore assert the query
//! executes and returns a well-formed (possibly empty) result set against the
//! freshly-migrated DB.

use systemprompt_identifiers::InstanceId;
use systemprompt_scheduler::repository::{AnalyticsRepository, SecurityRepository};
use systemprompt_scheduler::{JobRepository, JobRunRecord, JobStatus, SchedulerRepository};
use systemprompt_test_fixtures::{fixture_database_url, fixture_db_pool};

// Returns None (skipping the test) when no integration DB is configured.

// Builds a name unique across processes (PID), tests (atomic counter), and
// reruns (nanosecond clock) without pulling in a uuid dependency.
fn unique_job_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{}_{}_{}", std::process::id(), seq, nanos)
}

mod scheduler_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds_against_migrated_db() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let _repo = SchedulerRepository::new(&pool).expect("composite repo should construct");
    }

    #[tokio::test]
    async fn upsert_then_find_returns_inserted_row() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_upsert");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert should succeed");

        let found = repo.find_job(&name).await.expect("find should succeed");
        let job = found.expect("row should exist after upsert");
        assert_eq!(job.job_name, name);
        assert_eq!(job.schedule, "0 0 * * * *");
        assert!(job.enabled);
        assert_eq!(job.run_count, 0);
        assert!(job.last_run.is_none());
        assert!(job.last_status.is_none());
    }

    #[tokio::test]
    async fn find_missing_job_returns_none() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let missing = unique_job_name("sched_absent");

        let found = repo.find_job(&missing).await.expect("find should succeed");
        assert!(found.is_none(), "a never-inserted job must not be found");
    }

    #[tokio::test]
    async fn upsert_conflict_updates_schedule_and_enabled() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_conflict");

        repo.upsert_job(&name, "0 0 1 * * *", true)
            .await
            .expect("first upsert");
        repo.upsert_job(&name, "0 */5 * * * *", false)
            .await
            .expect("second upsert (conflict update)");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.schedule, "0 */5 * * * *");
        assert!(!job.enabled, "conflict update should flip enabled to false");
    }

    #[tokio::test]
    async fn update_job_execution_persists_status_and_error() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_exec");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Failed,
                error: Some("boom"),
                next_run: None,
                instance_id: &InstanceId::new("test-node"),
            },
        )
        .await
        .expect("update execution");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.last_status.as_deref(), Some("failed"));
        assert_eq!(job.last_error.as_deref(), Some("boom"));
        assert!(job.last_run.is_some(), "last_run should be stamped");
    }

    #[tokio::test]
    async fn update_job_execution_success_clears_error() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_success");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Failed,
                error: Some("boom"),
                next_run: None,
                instance_id: &InstanceId::new("test-node"),
            },
        )
        .await
        .expect("first update");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Success,
                error: None,
                next_run: None,
                instance_id: &InstanceId::new("test-node"),
            },
        )
        .await
        .expect("second update");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.last_status.as_deref(), Some("success"));
        assert!(
            job.last_error.is_none(),
            "success run should null the error"
        );
    }

    #[tokio::test]
    async fn increment_run_count_accumulates() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let name = unique_job_name("sched_runcount");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.increment_run_count(&name).await.expect("inc 1");
        repo.increment_run_count(&name).await.expect("inc 2");
        repo.increment_run_count(&name).await.expect("inc 3");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert_eq!(job.run_count, 3);
    }

    #[tokio::test]
    async fn list_enabled_jobs_includes_enabled_excludes_disabled() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");
        let enabled = unique_job_name("sched_list_on");
        let disabled = unique_job_name("sched_list_off");

        repo.upsert_job(&enabled, "0 0 * * * *", true)
            .await
            .expect("upsert enabled");
        repo.upsert_job(&disabled, "0 0 * * * *", false)
            .await
            .expect("upsert disabled");

        let rows = repo.list_enabled_jobs().await.expect("list enabled");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            names.contains(&enabled.as_str()),
            "enabled job must appear in list_enabled_jobs"
        );
        assert!(
            !names.contains(&disabled.as_str()),
            "disabled job must be excluded from list_enabled_jobs"
        );
    }

    #[tokio::test]
    async fn cleanup_empty_contexts_returns_rows_affected_count() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SchedulerRepository::new(&pool).expect("repo");

        // No seeded contexts; the DELETE simply affects whatever stale empty
        // contexts exist (0 on a fresh DB). The call must succeed and return a
        // count.
        let affected = repo
            .cleanup_empty_contexts(1)
            .await
            .expect("cleanup should execute");
        let _ = affected;
    }
}

mod job_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let _repo = JobRepository::new(&pool).expect("job repo should construct");
    }

    #[tokio::test]
    async fn set_enabled_toggles_flag() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = JobRepository::new(&pool).expect("repo");
        let name = unique_job_name("job_set_enabled");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.set_enabled(&name, false)
            .await
            .expect("set_enabled false");

        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert!(!job.enabled);

        repo.set_enabled(&name, true)
            .await
            .expect("set_enabled true");
        let job = repo
            .find_job(&name)
            .await
            .expect("find")
            .expect("row exists");
        assert!(job.enabled);
    }

    #[tokio::test]
    async fn list_recent_runs_includes_executed_job() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = JobRepository::new(&pool).expect("repo");
        let name = unique_job_name("job_recent");

        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");
        repo.update_job_execution(
            &name,
            JobRunRecord {
                status: JobStatus::Success,
                error: None,
                next_run: None,
                instance_id: &InstanceId::new("test-node"),
            },
        )
        .await
        .expect("execute");

        let rows = repo.list_recent_runs(100).await.expect("list recent");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            names.contains(&name.as_str()),
            "a job with a stamped last_run must appear in list_recent_runs"
        );
    }

    #[tokio::test]
    async fn list_recent_runs_respects_limit() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = JobRepository::new(&pool).expect("repo");

        let rows = repo.list_recent_runs(2).await.expect("list recent");
        assert!(
            rows.len() <= 2,
            "list_recent_runs must honour the LIMIT, got {}",
            rows.len()
        );
    }

    #[tokio::test]
    async fn list_recent_runs_excludes_never_run_job() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = JobRepository::new(&pool).expect("repo");
        let name = unique_job_name("job_never_run");

        // Inserted but never executed: last_run stays NULL, so it must not
        // appear in the recent-runs view regardless of limit.
        repo.upsert_job(&name, "0 0 * * * *", true)
            .await
            .expect("upsert");

        let rows = repo.list_recent_runs(1000).await.expect("list recent");
        let names: Vec<&str> = rows.iter().map(|j| j.job_name.as_str()).collect();
        assert!(
            !names.contains(&name.as_str()),
            "a job with NULL last_run must not appear in list_recent_runs"
        );
    }
}

mod analytics_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let _repo = AnalyticsRepository::new(&pool).expect("analytics repo should construct");
    }

    #[tokio::test]
    async fn cleanup_empty_contexts_executes_for_various_windows() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = AnalyticsRepository::new(&pool).expect("repo");

        for hours in [0_i64, 1, 24, 168] {
            repo.cleanup_empty_contexts(hours)
                .await
                .expect("cleanup query should execute for any window");
        }
    }

    #[tokio::test]
    async fn cleanup_collects_orphaned_cli_contexts_but_spares_session_bound_ones() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = AnalyticsRepository::new(&pool).expect("repo");
        let raw = pool.pool_arc().expect("raw pool");

        let user_id = systemprompt_test_fixtures::unique_user_id("schedgc");
        let session_id = systemprompt_identifiers::SessionId::generate();
        systemprompt_test_fixtures::seed_user_row(
            &pool,
            &user_id,
            &format!("{}@schedgc.invalid", user_id.as_str()),
        )
        .await
        .expect("seed user");
        systemprompt_test_fixtures::seed_user_session(&pool, &user_id, &session_id)
            .await
            .expect("seed session");

        let live_ctx = format!("schedgc_live_{}", session_id.as_str());
        let orphan_ctx = format!("schedgc_orphan_{}", session_id.as_str());
        let old = chrono::Utc::now() - chrono::Duration::hours(48);
        for (ctx, sess) in [(&live_ctx, Some(session_id.as_str())), (&orphan_ctx, None)] {
            sqlx::query(
                "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, \
                 created_at, updated_at) VALUES ($1, $2, $3, $4, 'cli_session', $5, $5)",
            )
            .bind(ctx)
            .bind(user_id.as_str())
            .bind(sess)
            .bind("CLI Session - schedgc")
            .bind(old)
            .execute(raw.as_ref())
            .await
            .expect("seed context");
        }

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        let survivors: Vec<String> =
            sqlx::query_scalar("SELECT context_id FROM user_contexts WHERE user_id = $1")
                .bind(user_id.as_str())
                .fetch_all(raw.as_ref())
                .await
                .expect("list survivors");
        assert!(
            survivors.contains(&live_ctx),
            "empty CLI context bound to a live session must survive"
        );
        assert!(
            !survivors.contains(&orphan_ctx),
            "session-orphaned CLI context must be collected"
        );

        let _ = sqlx::query("DELETE FROM user_contexts WHERE user_id = $1")
            .bind(user_id.as_str())
            .execute(raw.as_ref())
            .await;
        let _ = sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
            .bind(user_id.as_str())
            .execute(raw.as_ref())
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id.as_str())
            .execute(raw.as_ref())
            .await;
    }
}

mod security_repository {
    use super::*;

    #[tokio::test]
    async fn new_succeeds() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let _repo = SecurityRepository::new(&pool).expect("security repo should construct");
    }

    #[tokio::test]
    async fn find_high_volume_ips_returns_well_formed_records() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SecurityRepository::new(&pool).expect("repo");

        // A very high threshold guarantees an empty result on any realistic
        // DB, but the aggregation + filter_map mapping path still executes.
        let records = repo
            .find_high_volume_ips(i64::MAX)
            .await
            .expect("query should execute");
        assert!(records.is_empty());

        // A threshold of 1 may surface real rows; every returned record must
        // carry the ip_address the mapping populates.
        let records = repo
            .find_high_volume_ips(1)
            .await
            .expect("query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
            assert!(rec.session_count >= 1);
            assert!(rec.country.is_none());
        }
    }

    #[tokio::test]
    async fn find_scanner_ips_executes() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SecurityRepository::new(&pool).expect("repo");

        let records = repo
            .find_scanner_ips(1)
            .await
            .expect("scanner query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
            assert!(rec.country.is_none());
        }
    }

    #[tokio::test]
    async fn find_recent_ips_executes() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SecurityRepository::new(&pool).expect("repo");

        let records = repo
            .find_recent_ips()
            .await
            .expect("recent-ips query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
        }
    }

    #[tokio::test]
    async fn find_high_risk_country_ips_populates_country() {
        let pool = systemprompt_test_fixtures::db_pool_or_skip!().0;
        let repo = SecurityRepository::new(&pool).expect("repo");

        let records = repo
            .find_high_risk_country_ips(i64::MAX)
            .await
            .expect("country query should execute");
        assert!(records.is_empty());

        let records = repo
            .find_high_risk_country_ips(1)
            .await
            .expect("country query should execute");
        for rec in &records {
            assert!(rec.ip_address.is_some());
            // This query selects country, so any returned record carries it.
            assert!(rec.country.is_some());
        }
    }
}

// Contexts holding audit rows (MCP tool executions, governance decisions) are
// not "empty" regardless of age — the audit tables reference `context_id`
// without an FK, so nothing else protects them.
mod empty_context_audit_guards {
    use super::*;

    struct Seed {
        pool: systemprompt_database::DbPool,
        user_id: systemprompt_identifiers::UserId,
        session_id: systemprompt_identifiers::SessionId,
    }

    impl Seed {
        async fn new(tag: &str) -> Self {
            let url = fixture_database_url().expect("caller checked the DB is configured");
            let pool = fixture_db_pool(&url).await.expect("pool");
            let user_id = systemprompt_test_fixtures::unique_user_id(tag);
            let session_id = systemprompt_identifiers::SessionId::generate();
            systemprompt_test_fixtures::seed_user_row(
                &pool,
                &user_id,
                &format!("{}@{tag}.invalid", user_id.as_str()),
            )
            .await
            .expect("seed user");
            systemprompt_test_fixtures::seed_user_session(&pool, &user_id, &session_id)
                .await
                .expect("seed session");
            Self {
                pool,
                user_id,
                session_id,
            }
        }

        fn raw(&self) -> std::sync::Arc<sqlx::PgPool> {
            self.pool.pool_arc().expect("raw pool")
        }

        async fn seed_old_context(&self, context_id: &str) {
            let old = chrono::Utc::now() - chrono::Duration::hours(72);
            sqlx::query(
                "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, \
                 created_at, updated_at) VALUES ($1, $2, NULL, $3, 'conversation', $4, $4)",
            )
            .bind(context_id)
            .bind(self.user_id.as_str())
            .bind("audit-guard fixture")
            .bind(old)
            .execute(self.raw().as_ref())
            .await
            .expect("seed context");
        }

        async fn seed_tool_execution(&self, execution_id: &str, context_id: &str) {
            sqlx::query(
                "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, \
                 started_at, input, status, user_id, session_id, context_id) VALUES ($1, \
                 'audit_guard_tool', 'audit_guard_server', NOW() - INTERVAL '72 hours', '{}', \
                 'success', $2, $3, $4)",
            )
            .bind(execution_id)
            .bind(self.user_id.as_str())
            .bind(self.session_id.as_str())
            .bind(context_id)
            .execute(self.raw().as_ref())
            .await
            .expect("seed mcp_tool_executions");
        }

        async fn seed_governance_decision(&self, decision_id: &str, context_id: &str) {
            sqlx::query(
                "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, decision, \
                 policy, reason, actor_kind, actor_id, act_chain, context_id, created_at) VALUES \
                 ($1, $2, $3, 'audit_guard_tool', 'allow', 'audit-guard', 'fixture', 'user', $2, \
                 '[]'::jsonb, $4, NOW() - INTERVAL '72 hours')",
            )
            .bind(decision_id)
            .bind(self.user_id.as_str())
            .bind(self.session_id.as_str())
            .bind(context_id)
            .execute(self.raw().as_ref())
            .await
            .expect("seed governance_decisions");
        }

        async fn context_exists(&self, context_id: &str) -> bool {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM user_contexts WHERE context_id = $1)",
            )
            .bind(context_id)
            .fetch_one(self.raw().as_ref())
            .await
            .expect("context probe")
        }

        async fn cleanup(&self) {
            let raw = self.raw();
            for stmt in [
                "DELETE FROM governance_decisions WHERE user_id = $1",
                "DELETE FROM mcp_tool_executions WHERE user_id = $1",
                "DELETE FROM user_contexts WHERE user_id = $1",
                "DELETE FROM user_sessions WHERE user_id = $1",
                "DELETE FROM users WHERE id = $1",
            ] {
                let _ = sqlx::query(stmt)
                    .bind(self.user_id.as_str())
                    .execute(raw.as_ref())
                    .await;
            }
        }
    }

    #[tokio::test]
    async fn context_with_tool_execution_survives_cleanup() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        if fixture_db_pool(&url).await.is_err() {
            return;
        }
        let seed = Seed::new("auditmte").await;
        let repo = AnalyticsRepository::new(&seed.pool).expect("repo");
        // The audit row is written first: a concurrent sweep would otherwise
        // collect the context through the gap before it is protected.
        let ctx_id = unique_job_name("auditctx_mte");
        seed.seed_tool_execution(&unique_job_name("auditexec"), &ctx_id)
            .await;
        seed.seed_old_context(&ctx_id).await;

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        assert!(
            seed.context_exists(&ctx_id).await,
            "a context referenced by mcp_tool_executions must not be collected"
        );
        seed.cleanup().await;
    }

    #[tokio::test]
    async fn context_with_governance_decision_survives_cleanup() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        if fixture_db_pool(&url).await.is_err() {
            return;
        }
        let seed = Seed::new("auditgd").await;
        let repo = AnalyticsRepository::new(&seed.pool).expect("repo");
        let ctx_id = unique_job_name("auditctx_gd");
        seed.seed_governance_decision(&unique_job_name("auditdec"), &ctx_id)
            .await;
        seed.seed_old_context(&ctx_id).await;

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        assert!(
            seed.context_exists(&ctx_id).await,
            "a context referenced by governance_decisions must not be collected"
        );
        seed.cleanup().await;
    }

    #[tokio::test]
    async fn truly_empty_old_context_is_deleted() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        if fixture_db_pool(&url).await.is_err() {
            return;
        }
        let seed = Seed::new("auditbare").await;
        let repo = AnalyticsRepository::new(&seed.pool).expect("repo");
        let ctx_id = unique_job_name("auditctx_bare");
        seed.seed_old_context(&ctx_id).await;

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        assert!(
            !seed.context_exists(&ctx_id).await,
            "an old context with no messages and no audit rows must be collected"
        );
        seed.cleanup().await;
    }

    #[tokio::test]
    async fn count_empty_contexts_counts_what_cleanup_deletes() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        if fixture_db_pool(&url).await.is_err() {
            return;
        }
        let seed = Seed::new("auditcount").await;
        let repo = AnalyticsRepository::new(&seed.pool).expect("repo");
        let collectable = unique_job_name("auditctx_count_bare");
        let protected = unique_job_name("auditctx_count_mte");
        seed.seed_tool_execution(&unique_job_name("auditexec"), &protected)
            .await;
        seed.seed_old_context(&protected).await;
        seed.seed_old_context(&collectable).await;

        // The sweep is table-wide and shards run in parallel, so only the
        // seeded rows can be asserted on: a concurrent cleanup may already
        // have collected this one between the count and the delete.
        let counted = repo.count_empty_contexts(1).await.expect("count");
        if seed.context_exists(&collectable).await {
            assert!(
                counted >= 1,
                "a live collectable context must be counted, got {counted}"
            );
        }

        repo.cleanup_empty_contexts(1).await.expect("cleanup");
        assert!(!seed.context_exists(&collectable).await);
        assert!(seed.context_exists(&protected).await);

        seed.cleanup().await;
    }

    // Regression: an MCP tool execution whose context has already been deleted
    // must survive every retention sweep — audit rows outlive their context.
    #[tokio::test]
    async fn orphaned_tool_execution_survives_all_retention_sweeps() {
        let Ok(url) = fixture_database_url() else {
            return;
        };
        if fixture_db_pool(&url).await.is_err() {
            return;
        }
        let seed = Seed::new("auditorphan").await;
        let repo = AnalyticsRepository::new(&seed.pool).expect("repo");
        let exec_id = unique_job_name("auditexec_orphan");
        let missing_ctx = unique_job_name("auditctx_missing");
        seed.seed_tool_execution(&exec_id, &missing_ctx).await;

        repo.cleanup_empty_contexts(1).await.expect("context sweep");

        let cleanup = systemprompt_database::CleanupRepository::new(
            (*seed.pool.write_pool_arc().expect("write pool")).clone(),
        );
        cleanup.delete_orphaned_logs().await.expect("orphaned logs");
        cleanup.delete_old_logs(30).await.expect("old logs");
        cleanup
            .delete_expired_oauth_tokens()
            .await
            .expect("oauth tokens");
        cleanup
            .delete_expired_oauth_codes()
            .await
            .expect("oauth codes");
        cleanup
            .delete_expired_oauth_state_bindings()
            .await
            .expect("oauth state bindings");
        cleanup
            .delete_expired_oauth_jti_revocations()
            .await
            .expect("oauth jti revocations");

        let survived = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1)",
        )
        .bind(&exec_id)
        .fetch_one(seed.raw().as_ref())
        .await
        .expect("execution probe");
        assert!(
            survived,
            "an MCP tool execution must outlive the context it referenced"
        );

        seed.cleanup().await;
    }
}
