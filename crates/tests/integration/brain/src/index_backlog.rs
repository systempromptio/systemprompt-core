//! The `brain_fact_index` projection: an upgrade queues every stored fact
//! and the sweep drains the queue, restoring the working-set rows, links,
//! aliases and weighted vector and flooring `observed_at` to `created_at`,
//! before it checks the working set for drift.

use systemprompt_brain::index::BRAIN_FACT_INDEX;
use systemprompt_models::brain::FactScope;

use crate::events::entered;
use crate::harness::{Harness, Setup};
use crate::plane::{OWNERSHIP, kind};

#[tokio::test]
async fn the_sweep_drains_the_queued_facts_through_the_write_paths_indexer() {
    let h = Harness::with("brain_index_backlog", Setup::confirming()).await;
    let author = h.user("author", &[]).await;
    for product in ["acme", "globex"] {
        h.emit(&entered(
            &author,
            kind(OWNERSHIP),
            product,
            serde_json::json!({ "team": "ops" }),
            FactScope::Org,
        ))
        .await;
    }
    h.drain().await;
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM brain_facts ORDER BY id")
        .fetch_all(&h.pg)
        .await
        .expect("fact ids");
    assert_eq!(ids.len(), 2);

    for sql in [
        "DELETE FROM brain_working_set",
        "DELETE FROM brain_fact_links",
        "DELETE FROM brain_aliases",
        "UPDATE brain_facts SET search_weighted = ''::tsvector,
                                observed_at = created_at + interval '1 day'",
    ] {
        sqlx::query(sql)
            .execute(&h.pg)
            .await
            .expect("an upgraded store");
    }
    let queued: i64 = sqlx::query_scalar("SELECT queue_projection_backlog($1, $2)")
        .bind(BRAIN_FACT_INDEX)
        .bind(&ids)
        .fetch_one(&h.pg)
        .await
        .expect("queue");
    assert_eq!(queued, 2);

    let report = h
        .pipeline
        .sweep(chrono::Utc::now())
        .await
        .expect("sweep")
        .expect("this replica swept");
    assert_eq!(report.index_backlog.keys, 2, "{:?}", report.index_backlog);
    assert_eq!(report.index_backlog.remaining, 0);
    assert_eq!(report.index_backlog.requeued, 0);
    assert!(
        !report.working_set_rebuilt,
        "the drain restored the working set before the drift check"
    );
    assert!(
        report.working_set.is_consistent(),
        "{:?}",
        report.working_set
    );

    assert_eq!(h.total("brain_working_set").await, 2);
    assert_eq!(h.total("brain_fact_links").await, 2);
    assert!(h.total("brain_aliases").await >= 2);
    let unindexed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM brain_facts
         WHERE search_weighted = ''::tsvector OR observed_at <> created_at",
    )
    .fetch_one(&h.pg)
    .await
    .expect("count");
    assert_eq!(
        unindexed, 0,
        "every fact carries its vector and observed_at"
    );
    let depth: i64 = sqlx::query_scalar("SELECT projection_backlog_depth($1)")
        .bind(BRAIN_FACT_INDEX)
        .fetch_one(&h.pg)
        .await
        .expect("depth");
    assert_eq!(depth, 0);
    h.finish().await;
}
