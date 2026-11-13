//! DB-backed tests for job `execute()` bodies with pre-seeded data.
//!
//! Each test module seeds rows that push the job past the "nothing to do"
//! empty-DB path and into the scoring/flagging/banning decision branches.
//! Rows are cleaned up after each test so shards do not interfere.

use std::sync::Arc;

use systemprompt_scheduler::{BehavioralAnalysisJob, MaliciousIpBlacklistJob};
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_traits::{Dependencies, Job, JobContext};

fn make_test_ctx(pool: &systemprompt_database::DbPool, url: &str) -> JobContext {
    use systemprompt_identifiers::{Actor, UserId};

    let app_ctx = test_app_context(pool, url);

    let dependencies = Dependencies::new()
        .with(Arc::clone(app_ctx.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(app_ctx);

    let owner = UserId::new("seeded-job-test-admin");
    let actor = Actor::job(owner, "test".to_string());

    JobContext::new(actor, dependencies)
}

fn unique_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{}_{}_{}", std::process::id(), n, nanos)
}

fn unique_ip(class_b: &str) -> String {
    // Lower two octets carry the full PID, not `pid % N`: live PIDs are unique,
    // so parallel test processes never alias onto one address and race on the
    // shared `banned_ips` table.
    let pid = std::process::id();
    format!(
        "{class_b}.{}.{}",
        ((pid >> 8) & 0xff) as u8,
        (pid & 0xff) as u8
    )
}

mod behavioral_analysis_seeded {
    use super::*;

    #[tokio::test]
    async fn execute_flags_high_request_count_fingerprint() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_hireq");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                last_seen_at
            ) VALUES ($1, 200, 1, 5.0, 0, 50, 0, NOW())
            "#,
            hash,
        )
        .execute(&*pg)
        .await
        .expect("seed fingerprint_reputation row");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error with seeded fingerprint");

        assert!(
            result.success,
            "BehavioralAnalysisJob must report success even when flagging fingerprints"
        );

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();
    }

    #[tokio::test]
    async fn execute_counts_flagged_as_processed() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_counted");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                last_seen_at
            ) VALUES ($1, 150, 1, 5.0, 0, 50, 0, NOW())
            "#,
            hash,
        )
        .execute(&*pg)
        .await
        .expect("seed fingerprint_reputation row");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.items_processed.unwrap_or(0) >= 1,
            "at least one fingerprint above HIGH_REQUEST_THRESHOLD must be counted as processed"
        );

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();
    }

    #[tokio::test]
    async fn execute_flags_sustained_velocity_fingerprint() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_velocity");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                last_seen_at
            ) VALUES ($1, 50, 1, 15.0, 90, 50, 0, NOW())
            "#,
            hash,
        )
        .execute(&*pg)
        .await
        .expect("seed high-velocity fingerprint_reputation row");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed with a velocity-flagged fingerprint"
        );

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();
    }

    #[tokio::test]
    async fn execute_flags_excessive_sessions_fingerprint() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_sessions");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                last_seen_at
            ) VALUES ($1, 10, 15, 2.0, 0, 50, 0, NOW())
            "#,
            hash,
        )
        .execute(&*pg)
        .await
        .expect("seed high-session-count fingerprint_reputation row");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed with an excessive-sessions fingerprint"
        );

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();
    }

    #[tokio::test]
    async fn execute_triggers_ban_path_for_abuse_threshold_crossed() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_ban");
        let ip = unique_ip("10.0");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                last_ip_address,
                last_seen_at
            ) VALUES ($1, 200, 1, 5.0, 0, 50, 5, $2, NOW())
            "#,
            hash,
            ip,
        )
        .execute(&*pg)
        .await
        .expect("seed fingerprint_reputation row with high abuse_incidents");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must report success even when banning IPs"
        );

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();

        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_handles_reputation_decay_below_threshold() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_decay");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                is_flagged,
                last_seen_at
            ) VALUES ($1, 5, 1, 1.0, 0, 15, 0, false, NOW())
            "#,
            hash,
        )
        .execute(&*pg)
        .await
        .expect("seed low-reputation fingerprint_reputation row");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(result.success, "job must succeed on reputation-decay path");

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();
    }

    #[tokio::test]
    async fn execute_skips_ban_when_no_ip_address() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let hash = unique_id("fp_noip");

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation (
                fingerprint_hash,
                total_request_count,
                total_session_count,
                peak_requests_per_minute,
                sustained_high_velocity_minutes,
                reputation_score,
                abuse_incidents,
                last_ip_address,
                last_seen_at
            ) VALUES ($1, 200, 1, 5.0, 0, 50, 5, NULL, NOW())
            "#,
            hash,
        )
        .execute(&*pg)
        .await
        .expect("seed fingerprint_reputation row without ip");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed when ban path is short-circuited by missing ip"
        );

        sqlx::query!(
            "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            hash
        )
        .execute(&*pg)
        .await
        .ok();
    }

    #[tokio::test]
    async fn execute_multiple_fingerprints_all_branches() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let fp_high_req = unique_id("fp_multi_req");
        let fp_velocity = unique_id("fp_multi_vel");
        let fp_clean = unique_id("fp_multi_clean");
        let hashes = [fp_high_req.clone(), fp_velocity.clone(), fp_clean.clone()];

        sqlx::query!(
            r#"
            INSERT INTO fingerprint_reputation
                (fingerprint_hash, total_request_count, total_session_count,
                 peak_requests_per_minute, sustained_high_velocity_minutes,
                 reputation_score, abuse_incidents, last_seen_at)
            VALUES
                ($1, 200, 1, 5.0, 0, 50, 0, NOW()),
                ($2, 50, 1, 15.0, 90, 50, 0, NOW()),
                ($3, 5, 1, 1.0, 0, 80, 0, NOW())
            "#,
            fp_high_req,
            fp_velocity,
            fp_clean,
        )
        .execute(&*pg)
        .await
        .expect("seed multiple fingerprint_reputation rows");

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = BehavioralAnalysisJob
            .execute(&ctx)
            .await
            .expect("execute must not error with multiple fingerprints");

        assert!(
            result.success,
            "job must succeed with multiple seeded fingerprints"
        );
        assert!(
            result.items_processed.unwrap_or(0) >= 2,
            "at least the two threshold-crossing fingerprints must be counted"
        );

        for h in &hashes {
            sqlx::query!(
                "DELETE FROM fingerprint_reputation WHERE fingerprint_hash = $1",
                h
            )
            .execute(&*pg)
            .await
            .ok();
        }
    }
}

mod malicious_ip_blacklist_seeded {
    use super::*;

    async fn insert_session(
        pg: &sqlx::PgPool,
        session_id: &str,
        ip: &str,
        is_scanner: bool,
        is_bot: bool,
        country: Option<&str>,
    ) {
        sqlx::query!(
            r#"
            INSERT INTO user_sessions (
                session_id, ip_address, is_scanner, is_bot, country,
                started_at, last_activity_at
            )
            VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
            ON CONFLICT (session_id) DO NOTHING
            "#,
            session_id,
            ip,
            is_scanner,
            is_bot,
            country,
        )
        .execute(pg)
        .await
        .expect("seed user_sessions row");
    }

    #[tokio::test]
    async fn execute_bans_high_volume_ip() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let ip = unique_ip("192.168");
        let mut session_ids = Vec::new();

        for i in 0u64..110 {
            let sid = unique_id(&format!("hvol_sess_{i}"));
            insert_session(&pg, &sid, &ip, false, false, None).await;
            session_ids.push(sid);
        }

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must report success after banning a high-volume IP"
        );

        assert!(
            result.items_processed.unwrap_or(0) >= 1,
            "at least one IP should have been banned"
        );

        for sid in &session_ids {
            sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
                .execute(&*pg)
                .await
                .ok();
        }
        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_bans_scanner_ip() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let ip = unique_ip("172.16");
        let mut session_ids = Vec::new();

        for i in 0u64..5 {
            let sid = unique_id(&format!("scanner_sess_{i}"));
            insert_session(&pg, &sid, &ip, true, false, None).await;
            session_ids.push(sid);
        }

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must report success after banning a scanner IP"
        );

        for sid in &session_ids {
            sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
                .execute(&*pg)
                .await
                .ok();
        }
        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_bans_datacenter_ip() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let ip = unique_ip("47.79");
        let sid = unique_id("datacenter_sess");

        insert_session(&pg, &sid, &ip, false, false, None).await;

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed after processing datacenter IP"
        );

        sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
            .execute(&*pg)
            .await
            .ok();
        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_bans_high_risk_country_ip() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let ip = unique_ip("10.20");
        let mut session_ids = Vec::new();

        for i in 0u64..6 {
            let sid = unique_id(&format!("hrcountry_sess_{i}"));
            insert_session(&pg, &sid, &ip, false, false, Some("CN")).await;
            session_ids.push(sid);
        }

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed after processing high-risk country IP"
        );

        for sid in &session_ids {
            sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
                .execute(&*pg)
                .await
                .ok();
        }
        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_skips_already_banned_ip() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let ip = unique_ip("192.0");

        sqlx::query!(
            r#"
            INSERT INTO banned_ips (ip_address, reason, expires_at, ban_source)
            VALUES ($1, 'pre-existing test ban', NOW() + INTERVAL '7 days', 'manual')
            ON CONFLICT (ip_address) DO NOTHING
            "#,
            ip,
        )
        .execute(&*pg)
        .await
        .expect("pre-seed banned_ips row");

        let mut session_ids = Vec::new();
        for i in 0u64..110 {
            let sid = unique_id(&format!("already_banned_sess_{i}"));
            insert_session(&pg, &sid, &ip, false, false, None).await;
            session_ids.push(sid);
        }

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed even when an IP is already banned"
        );

        for sid in &session_ids {
            sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
                .execute(&*pg)
                .await
                .ok();
        }
        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_handles_non_high_risk_country_not_banned() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let ip = unique_ip("10.30");
        let mut session_ids = Vec::new();

        for i in 0u64..6 {
            let sid = unique_id(&format!("lowrisk_sess_{i}"));
            insert_session(&pg, &sid, &ip, false, false, Some("US")).await;
            session_ids.push(sid);
        }

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed when country is not in high-risk list"
        );

        for sid in &session_ids {
            sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
                .execute(&*pg)
                .await
                .ok();
        }
        sqlx::query!("DELETE FROM banned_ips WHERE ip_address = $1", ip)
            .execute(&*pg)
            .await
            .ok();
    }

    #[tokio::test]
    async fn execute_processes_sessions_without_ip_gracefully() {
        let url = test_database_url();
        let pool = test_db_pool().await;
        let pg = pool.write_pool();

        let mut session_ids = Vec::new();
        for i in 0u64..5 {
            let sid = unique_id(&format!("nullip_sess_{i}"));
            sqlx::query!(
                r#"
                INSERT INTO user_sessions (
                    session_id, ip_address, is_scanner, is_bot, started_at, last_activity_at
                )
                VALUES ($1, NULL, true, false, NOW(), NOW())
                ON CONFLICT (session_id) DO NOTHING
                "#,
                sid,
            )
            .execute(&*pg)
            .await
            .expect("seed session with null ip");
            session_ids.push(sid);
        }

        let ctx = make_test_ctx(&pool, &url).with_enforce(true);
        let result = MaliciousIpBlacklistJob
            .execute(&ctx)
            .await
            .expect("execute must not error");

        assert!(
            result.success,
            "job must succeed even when scanner sessions have null ip_address"
        );

        for sid in &session_ids {
            sqlx::query!("DELETE FROM user_sessions WHERE session_id = $1", sid)
                .execute(&*pg)
                .await
                .ok();
        }
    }
}
