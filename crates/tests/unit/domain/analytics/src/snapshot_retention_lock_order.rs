use super::*;

#[tokio::test]
async fn owner_deletion_precedes_both_retention_entry_points_without_child_lock_inversion() {
    for all_owners in [false, true] {
        let (database, db, f) = isolated().await;
        let victim = format!("retention-delete-{}", f.owner);
        sqlx::query("INSERT INTO users(id,name,email) VALUES($1,$1,$2)")
            .bind(&victim)
            .bind(format!("{victim}@retention.invalid"))
            .execute(&f.pool)
            .await
            .unwrap();
        drain_reporting(&db).await.unwrap();
        let now = Utc::now();
        f.repository
            .submit(&f.owner, &request("ready", 1, now, 1))
            .await
            .unwrap();
        refresh(&f, now).await;
        let mut deletion = f.pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE users IN ROW EXCLUSIVE MODE")
            .execute(&mut *deletion)
            .await
            .unwrap();
        sqlx::query("LOCK TABLE analytics_fact_backfills IN ROW SHARE MODE")
            .execute(&mut *deletion)
            .await
            .unwrap();
        let pool = f.pool.clone();
        let owner = f.owner.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        let retention = tokio::spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
                .fetch_one(&mut *tx)
                .await
                .unwrap();
            started.send(pid).unwrap();
            let result = if all_owners {
                FeedbackSnapshotsRepository::compact_all_in(&mut tx, Utc::now())
                    .await
                    .map(|_| ())
            } else {
                FeedbackSnapshotsRepository::compact_in(&mut tx, &owner, Utc::now())
                    .await
                    .map(|_| ())
            };
            tx.rollback().await.unwrap();
            result
        });
        let pid = ready.await.unwrap();
        let waiting_on = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            loop {
                let blocked: Option<String> = sqlx::query_scalar("SELECT relation::regclass::text FROM pg_locks WHERE pid=$1 AND locktype='relation' AND NOT granted LIMIT 1")
                    .bind(pid).fetch_optional(&f.pool).await.unwrap();
                if let Some(relation) = blocked { break relation; }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }).await.expect("retention must reach its owner-deletion barrier");
        assert_eq!(
            waiting_on, "users",
            "retention must not acquire checkpoints before a deletion finishes with backfills"
        );
        let child_locks: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_locks WHERE pid=$1 AND granted AND mode='ExclusiveLock' AND relation IN ('analytics_fact_backfills'::regclass,'analytics_fact_checkpoints'::regclass)")
            .bind(pid).fetch_one(&f.pool).await.unwrap();
        assert_eq!(child_locks, 0);
        let removed = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            sqlx::query("DELETE FROM users WHERE id=$1")
                .bind(&victim)
                .execute(&mut *deletion),
        )
        .await
        .expect("actual owner deletion must progress while retention waits")
        .unwrap();
        assert_eq!(removed.rows_affected(), 1);
        deletion.commit().await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), retention)
            .await
            .expect("retention must leave the owner barrier after deletion commits")
            .unwrap();
        // The committed deletion is pending evidence; retention delivers it
        // itself instead of failing on it, and nothing is left for the worker.
        result.expect("retention delivers the committed deletion in its own transaction");
        assert_eq!(drain_reporting(&db).await.unwrap(), 1);
        let mut retry = f.pool.begin().await.unwrap();
        if all_owners {
            FeedbackSnapshotsRepository::compact_all_in(&mut retry, Utc::now())
                .await
                .unwrap();
        } else {
            FeedbackSnapshotsRepository::compact_in(&mut retry, &f.owner, Utc::now())
                .await
                .unwrap();
        }
        retry.commit().await.unwrap();
        cleanup(database, db).await;
    }
}
