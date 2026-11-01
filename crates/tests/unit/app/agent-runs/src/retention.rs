use chrono::{TimeZone, Utc};
use systemprompt_agent_runs::repository::retention::ExpiredRunDir;
use systemprompt_identifiers::AgentRunId;

use crate::repository::{Fixture, fixture, queue};

async fn finish(f: &Fixture, id: &AgentRunId, year: i32, run_dir: Option<&str>) {
    let finished_at = Utc.with_ymd_and_hms(year, 1, 1, 0, 0, 0).unwrap();
    sqlx::query("UPDATE agent_runs SET finished_at = $2, run_dir = $3 WHERE id = $1")
        .bind(id.as_str())
        .bind(finished_at)
        .bind(run_dir)
        .execute(f.repo.write_pool())
        .await
        .expect("finish run");
}

fn ours<'a>(rows: &'a [ExpiredRunDir], ids: &[&AgentRunId]) -> Vec<&'a ExpiredRunDir> {
    rows.iter().filter(|row| ids.contains(&&row.id)).collect()
}

#[tokio::test]
async fn expired_dirs_list_oldest_first_and_clear_drops_them() {
    let f = fixture().await;
    let oldest = queue(&f).await;
    let older = queue(&f).await;
    let dirless = queue(&f).await;
    let recent = queue(&f).await;
    finish(&f, &oldest, 1971, Some("/runs/oldest")).await;
    finish(&f, &older, 1972, Some("/runs/older")).await;
    finish(&f, &dirless, 1971, None).await;
    finish(&f, &recent, 2025, Some("/runs/recent")).await;
    let cutoff = Utc.with_ymd_and_hms(1980, 1, 1, 0, 0, 0).unwrap();
    let ids = [&oldest, &older, &dirless, &recent];

    let rows = f
        .repo
        .list_expired_run_dirs(cutoff, 1000)
        .await
        .expect("list expired");
    let listed = ours(&rows, &ids);
    assert_eq!(
        listed,
        vec![
            &ExpiredRunDir {
                id: oldest.clone(),
                run_dir: "/runs/oldest".to_owned(),
            },
            &ExpiredRunDir {
                id: older.clone(),
                run_dir: "/runs/older".to_owned(),
            },
        ]
    );

    f.repo.clear_run_dir(&oldest).await.expect("clear");
    let rows = f
        .repo
        .list_expired_run_dirs(cutoff, 1000)
        .await
        .expect("list after clear");
    let listed: Vec<&AgentRunId> = ours(&rows, &ids).iter().map(|row| &row.id).collect();
    assert_eq!(listed, vec![&older]);
}

#[tokio::test]
async fn limit_bounds_the_listing() {
    let f = fixture().await;
    let first = queue(&f).await;
    let second = queue(&f).await;
    finish(&f, &first, 1970, Some("/runs/a")).await;
    finish(&f, &second, 1970, Some("/runs/b")).await;
    let cutoff = Utc.with_ymd_and_hms(1980, 1, 1, 0, 0, 0).unwrap();

    let rows = f
        .repo
        .list_expired_run_dirs(cutoff, 1)
        .await
        .expect("list limited");
    assert_eq!(rows.len(), 1);
}
