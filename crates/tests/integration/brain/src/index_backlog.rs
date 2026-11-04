//! The `brain_fact_index` projection: an upgrade queues every stored fact
//! and the sweep drains the queue, restoring the working-set rows, links,
//! aliases and weighted vector and flooring `observed_at` to `created_at`,
//! before it checks the working set for drift. A key a constraint refuses
//! is parked past the retry budget on its own and the keys beside it are
//! still refreshed; any other failure re-queues the batch one attempt
//! dearer, since nothing unparks a key.

use systemprompt_brain::index::BRAIN_FACT_INDEX;
use systemprompt_database::projection::PARKED_ATTEMPTS;
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

#[tokio::test]
async fn a_key_the_index_refuses_is_parked_and_the_batch_still_lands() {
    let h = Harness::with("brain_index_backlog_parked", Setup::confirming()).await;
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
    for sql in [
        "DELETE FROM brain_subjects WHERE subject_id = 'globex'",
        "ALTER TABLE brain_subjects ADD CONSTRAINT brain_subjects_test_poison
         CHECK (subject_id <> 'globex')",
        "DELETE FROM brain_working_set",
        "DELETE FROM brain_fact_links",
        "DELETE FROM brain_aliases",
        "UPDATE brain_facts SET search_weighted = ''::tsvector",
    ] {
        sqlx::query(sql)
            .execute(&h.pg)
            .await
            .expect("an upgraded store with one refused subject");
    }
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM brain_facts ORDER BY id")
        .fetch_all(&h.pg)
        .await
        .expect("fact ids");
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
    assert_eq!(
        report.index_backlog.requeued, 0,
        "nothing was re-queued whole"
    );
    assert_eq!(
        report.index_backlog.remaining, 1,
        "the refused key is parked"
    );

    let parked = sqlx::query_as::<_, (String, i32)>(
        "SELECT b.key, b.attempts FROM projection_backlog b WHERE b.projection = $1",
    )
    .bind(BRAIN_FACT_INDEX)
    .fetch_all(&h.pg)
    .await
    .expect("parked keys");
    let globex = h.only_fact("globex").await;
    assert_eq!(parked, vec![(globex.id.to_string(), PARKED_ATTEMPTS)]);
    let acme = h.only_fact("acme").await;
    let indexed: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM brain_facts WHERE search_weighted <> ''::tsvector ORDER BY id",
    )
    .fetch_all(&h.pg)
    .await
    .expect("indexed");
    assert_eq!(
        indexed,
        vec![acme.id.to_string()],
        "the key beside it landed"
    );
    h.finish().await;
}

#[tokio::test]
async fn a_failure_that_is_not_a_constraint_requeues_the_batch_instead_of_parking() {
    let h = Harness::with("brain_index_backlog_requeue", Setup::confirming()).await;
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
    for sql in [
        "DELETE FROM brain_subjects WHERE subject_id = 'globex'",
        "CREATE FUNCTION brain_test_refuse() RETURNS trigger LANGUAGE plpgsql AS $$
         BEGIN
             IF NEW.subject_id = 'globex' THEN RAISE EXCEPTION 'not now'; END IF;
             RETURN NEW;
         END $$",
        "CREATE TRIGGER brain_test_refuse BEFORE INSERT ON brain_subjects
         FOR EACH ROW EXECUTE FUNCTION brain_test_refuse()",
        "DELETE FROM brain_working_set",
        "DELETE FROM brain_fact_links",
        "DELETE FROM brain_aliases",
        "UPDATE brain_facts SET search_weighted = ''::tsvector",
    ] {
        sqlx::query(sql)
            .execute(&h.pg)
            .await
            .expect("an upgraded store whose subject insert fails for a reason of its own");
    }
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM brain_facts ORDER BY id")
        .fetch_all(&h.pg)
        .await
        .expect("fact ids");
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
    assert_eq!(report.index_backlog.keys, 0, "{:?}", report.index_backlog);
    assert_eq!(
        report.index_backlog.requeued, 2,
        "the whole batch is re-queued"
    );
    assert_eq!(report.index_backlog.remaining, 2);
    let attempts: Vec<i32> =
        sqlx::query_scalar("SELECT attempts FROM projection_backlog WHERE projection = $1")
            .bind(BRAIN_FACT_INDEX)
            .fetch_all(&h.pg)
            .await
            .expect("attempts");
    assert_eq!(attempts, vec![1, 1], "one attempt dearer, nothing parked");
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_facts WHERE search_weighted <> ''::tsvector AND id <> $1",
            "none"
        )
        .await,
        0,
        "the batch rolled back whole"
    );
    h.finish().await;
}
