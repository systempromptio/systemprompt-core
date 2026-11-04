//! The consumer round's set-based reindex writes exactly what the per-event
//! path writes: a round of N entered facts ends with the same working-set
//! rows, links, aliases and weighted vectors as N single-event drains, and
//! the projection drain restores the same rows after an upgrade wipes them.
//! The set is three contacts, two of which share the alias "Jane Doe" so
//! the later one must win it, and one ownership fact of another kind. The
//! owner of a shared alias is the fact with the greatest `(updated_at,
//! id)`, so a full reindex after an upgrade lands it on the same contact.
//!
//! A round whose set reindex a constraint refuses runs again per event:
//! the refused event is recorded failed and leaves no fact, the others
//! keep their facts and indexes, and the model is not asked twice.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use systemprompt_brain::consumer::{BacklogSnapshot, BrainConsumer};
use systemprompt_brain::index::BRAIN_FACT_INDEX;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_manifest::profile::BrainConsumerProfile;
use systemprompt_models::brain::{BrainEvent, EnteredFact, EnteredStatus, FactScope, SubjectKey};
use systemprompt_traits::{Dependencies, JobContext, RunnerService};
use tokio_util::sync::CancellationToken;

use crate::events::{Landed, entered, price};
use crate::fact_index::{CONTACT, declared_with_contact};
use crate::harness::{Harness, Setup};
use crate::plane::{self, OWNERSHIP, kind};

const PROCESSED: &str =
    "SELECT COUNT(*) FROM event_outbox WHERE processed_at IS NOT NULL AND id <> $1";

/// Every derived row keyed by what both databases share: the subject id of
/// the fact, never its generated id.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Derived {
    working_set: Vec<(String, String)>,
    links: Vec<(String, String, String, String)>,
    aliases: Vec<(String, String, String, String)>,
    vectors: Vec<(String, String)>,
}

async fn derived(h: &Harness) -> Derived {
    let working_set = sqlx::query_as::<_, (String, String)>(
        "SELECT w.scope_key, f.subject_id FROM brain_working_set w
         JOIN brain_facts f ON f.id = w.fact_id ORDER BY 1, 2",
    )
    .fetch_all(&h.pg)
    .await
    .expect("working set");
    let links = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT f.subject_id, l.via_field, l.subject_kind, l.subject_id FROM brain_fact_links l
         JOIN brain_facts f ON f.id = l.fact_id ORDER BY 1, 2, 3, 4",
    )
    .fetch_all(&h.pg)
    .await
    .expect("links");
    let aliases = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT subject_kind, subject_id, alias, alias_norm FROM brain_aliases
         ORDER BY subject_kind, alias_norm COLLATE \"C\"",
    )
    .fetch_all(&h.pg)
    .await
    .expect("aliases");
    let vectors = sqlx::query_as::<_, (String, String)>(
        "SELECT subject_id, search_weighted::text FROM brain_facts ORDER BY 1",
    )
    .fetch_all(&h.pg)
    .await
    .expect("vectors");
    Derived {
        working_set,
        links,
        aliases,
        vectors,
    }
}

fn one_round() -> Setup {
    let mut setup = Setup::confirming();
    setup.profile.consumer = BrainConsumerProfile {
        batch: 32,
        poll_secs: 3600,
        sweep_secs: 3600,
        ..BrainConsumerProfile::default()
    };
    setup
}

fn contact(subject: &str, name: &str, account: &str, note: &str) -> EnteredFact {
    EnteredFact {
        kind: kind(CONTACT),
        subject: SubjectKey::new("client", subject),
        fields: serde_json::Map::from_iter([
            ("name".to_owned(), serde_json::json!(name)),
            ("account".to_owned(), serde_json::json!(account)),
            ("owner".to_owned(), serde_json::json!("user:u_42")),
            ("note".to_owned(), serde_json::json!(note)),
        ]),
        scope: FactScope::Org,
        topics: Vec::new(),
        status: EnteredStatus::Proposed,
        observed_at: None,
    }
}

fn events(author: &UserId) -> Vec<BrainEvent> {
    let now = Utc::now();
    vec![
        contact(
            "acme-rockets",
            "Jane Doe",
            "acme-holdings",
            "sends the bill monthly",
        )
        .to_event(author.clone(), now)
        .expect("entered event"),
        contact(
            "globex",
            "Hank Scorpio",
            "globex-group",
            "invoice on receipt",
        )
        .to_event(author.clone(), now)
        .expect("entered event"),
        contact(
            "initech",
            "jane doe",
            "initech-llc",
            "statement of account quarterly",
        )
        .to_event(author.clone(), now)
        .expect("entered event"),
        entered(
            author,
            kind(OWNERSHIP),
            "widget",
            serde_json::json!({ "team": "ops" }),
            FactScope::Org,
        ),
    ]
}

async fn emit_all(h: &Harness, author: &UserId) -> usize {
    let events = events(author);
    for event in &events {
        h.emit(event).await;
    }
    events.len()
}

async fn run_consumer_until(h: &Harness, at_least: i64) -> BacklogSnapshot {
    let consumer: Arc<BrainConsumer> = Arc::new(h.consumer());
    let shutdown = CancellationToken::new();
    let runner = {
        let consumer = Arc::clone(&consumer);
        let shutdown = shutdown.clone();
        let ctx = JobContext::new(Actor::user(UserId::generate()), Dependencies::new());
        tokio::spawn(async move { consumer.run(ctx, shutdown).await })
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline && h.count(PROCESSED, "none").await < at_least {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    shutdown.cancel();
    runner.await.expect("consumer stops");
    consumer.backlog()
}

async fn run_consumer_for(h: &Harness, window: Duration) -> BacklogSnapshot {
    let consumer: Arc<BrainConsumer> = Arc::new(h.consumer());
    let shutdown = CancellationToken::new();
    let runner = {
        let consumer = Arc::clone(&consumer);
        let shutdown = shutdown.clone();
        let ctx = JobContext::new(Actor::user(UserId::generate()), Dependencies::new());
        tokio::spawn(async move { consumer.run(ctx, shutdown).await })
    };
    tokio::time::sleep(window).await;
    shutdown.cancel();
    runner.await.expect("consumer stops");
    consumer.backlog()
}

#[tokio::test]
async fn a_consumer_round_indexes_exactly_what_single_event_drains_do() {
    let batched = Harness::with("brain_reindex_round", one_round()).await;
    plane::apply(&batched.pg, &declared_with_contact()).await;
    let author = batched.user("author", &[]).await;
    let emitted = emit_all(&batched, &author).await;
    let backlog = run_consumer_until(&batched, i64::try_from(emitted).expect("count")).await;
    assert_eq!(backlog.processed, u64::try_from(emitted).expect("count"));
    assert_eq!(backlog.failed, 0);
    assert_eq!(backlog.retried, 0);
    assert_eq!(
        backlog.reindexed,
        u64::try_from(emitted).expect("count"),
        "every landed fact was reindexed by the round"
    );
    let from_round = derived(&batched).await;

    let single = Harness::with("brain_reindex_single", Setup::confirming()).await;
    plane::apply(&single.pg, &declared_with_contact()).await;
    let author = single.user("author", &[]).await;
    emit_all(&single, &author).await;
    let drained = single.drain().await;
    assert_eq!(drained.len(), emitted);
    let from_events = derived(&single).await;

    assert_eq!(from_round, from_events);
    assert_eq!(from_round.vectors.len(), emitted);
    assert!(
        from_round.aliases.contains(&(
            "client".to_owned(),
            "initech".to_owned(),
            "jane doe".to_owned(),
            "jane doe".to_owned()
        )),
        "the later fact's spelling of a shared alias wins: {:?}",
        from_round.aliases
    );
    batched.finish().await;
    single.finish().await;
}

#[tokio::test]
async fn the_projection_drain_restores_what_the_round_wrote() {
    let h = Harness::with("brain_reindex_projection", one_round()).await;
    plane::apply(&h.pg, &declared_with_contact()).await;
    let author = h.user("author", &[]).await;
    let emitted = emit_all(&h, &author).await;
    run_consumer_until(&h, i64::try_from(emitted).expect("count")).await;
    let before = derived(&h).await;
    assert_eq!(before.vectors.len(), emitted);

    for sql in [
        "DELETE FROM brain_working_set",
        "DELETE FROM brain_fact_links",
        "DELETE FROM brain_aliases",
        "UPDATE brain_facts SET search_weighted = ''::tsvector",
    ] {
        sqlx::query(sql)
            .execute(&h.pg)
            .await
            .expect("an upgraded store");
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
    assert_eq!(queued, i64::try_from(emitted).expect("count"));

    let report = h
        .pipeline
        .sweep(Utc::now())
        .await
        .expect("sweep")
        .expect("this replica swept");
    assert_eq!(
        report.index_backlog.keys,
        u64::try_from(emitted).expect("count")
    );
    assert_eq!(report.index_backlog.requeued, 0);
    assert_eq!(derived(&h).await, before);
    h.finish().await;
}

#[tokio::test]
async fn two_events_for_one_identity_in_one_round_resolve_to_one_revised_fact() {
    let h = Harness::with("brain_reindex_identity", one_round()).await;
    plane::apply(&h.pg, &declared_with_contact()).await;
    let author = h.user("author", &[]).await;
    let now = Utc::now();
    let events = [
        contact(
            "acme-rockets",
            "Jane Doe",
            "acme-holdings",
            "sends the bill monthly",
        ),
        contact(
            "acme-rockets",
            "Jane Doe",
            "acme-holdings",
            "sends the bill weekly",
        ),
        contact(
            "acme-rockets",
            "Jane Doe",
            "acme-holdings",
            "sends the bill weekly",
        ),
    ];
    let mut ids = Vec::new();
    for entered in &events {
        let event = entered
            .to_event(author.clone(), now)
            .expect("entered event");
        ids.push(h.emit(&event).await);
    }
    let backlog = run_consumer_until(&h, 3).await;
    assert_eq!(backlog.processed, 3);
    assert_eq!(backlog.retried, 0);
    assert_eq!(backlog.failed, 0);

    let fact = h.only_fact("acme-rockets").await;
    assert_eq!(fact.revision, 2, "insert, revise, unchanged: {fact:?}");
    assert!(fact.statement.contains("weekly"), "{}", fact.statement);
    assert_eq!(h.total("brain_fact_revisions").await, 2);
    for id in &ids {
        assert_eq!(h.count(PROCESSED, "none").await, 3);
        let outcome: Option<String> =
            sqlx::query_scalar("SELECT outcome FROM brain_event_results WHERE outbox_id = $1")
                .bind(id.as_str())
                .fetch_one(&h.pg)
                .await
                .expect("result row");
        assert_eq!(outcome.as_deref(), Some("processed"), "{id}");
    }
    assert_eq!(
        backlog.reindexed, 1,
        "the one fact the round landed on is reindexed once"
    );
    h.finish().await;
}

const UNPROCESSED: &str =
    "SELECT COUNT(*) FROM event_outbox WHERE processed_at IS NULL AND id <> $1";
const INDEXED: &str =
    "SELECT COUNT(*) FROM brain_facts WHERE search_weighted <> ''::tsvector AND id <> $1";
const POISON: &str = "poison-inc";

#[tokio::test]
async fn a_fact_a_constraint_refuses_fails_alone_and_leaves_nothing_behind() {
    let h = Harness::with("brain_reindex_poison", one_round()).await;
    plane::apply(&h.pg, &declared_with_contact()).await;
    sqlx::query(
        "ALTER TABLE brain_subjects ADD CONSTRAINT brain_subjects_test_poison
         CHECK (subject_id <> 'poison-inc')",
    )
    .execute(&h.pg)
    .await
    .expect("a constraint entry validation does not know");
    let author = h.user("author", &[]).await;
    let good = emit_all(&h, &author).await;
    h.model
        .push(serde_json::json!([price("acme", "emea", "1200")]));
    let document = h.emit(&Landed::new().event(&author)).await;
    let poison = h
        .emit(
            &contact(
                POISON,
                "Poison Inc",
                "nowhere",
                "its subject row is refused",
            )
            .to_event(author.clone(), Utc::now())
            .expect("entered event"),
        )
        .await;
    let good = i64::try_from(good + 1).expect("count");
    let emitted = good + 1;

    let backlog = run_consumer_until(&h, emitted).await;
    assert_eq!(backlog.processed, u64::try_from(good).expect("count"));
    assert_eq!(backlog.failed, 1, "the refused event alone is failed");
    assert_eq!(backlog.retried, 0);
    assert_eq!(backlog.abandoned, 0);
    assert_eq!(
        h.count(INDEXED, "none").await,
        good,
        "every other fact is indexed"
    );
    assert_eq!(h.total("brain_subjects").await, good);
    assert!(
        h.fact_ids(POISON).await.is_empty(),
        "no fact survives a failed event"
    );
    assert_eq!(
        h.count(UNPROCESSED, "none").await,
        0,
        "the round committed every event"
    );
    assert_eq!(
        h.model.calls(),
        1,
        "the model was asked once for the document"
    );
    assert_eq!(
        h.count(
            "SELECT COUNT(*) FROM brain_extractions WHERE outbox_id = $1",
            document.as_str()
        )
        .await,
        1,
        "the extraction spend survived the second pass"
    );

    let outcome: Option<String> =
        sqlx::query_scalar("SELECT outcome FROM brain_event_results WHERE outbox_id = $1")
            .bind(poison.as_str())
            .fetch_one(&h.pg)
            .await
            .expect("result row");
    assert_eq!(outcome.as_deref(), Some("failed"));
    let attempts = sqlx::query_as::<_, (String, i32)>(
        "SELECT stage, attempt FROM brain_failures WHERE outbox_id = $1 ORDER BY attempt",
    )
    .bind(poison.as_str())
    .fetch_all(&h.pg)
    .await
    .expect("attempt rows");
    assert_eq!(attempts, vec![("process".to_owned(), 1)]);
    assert_eq!(
        h.total("brain_failures").await,
        1,
        "the rolled-back first pass left no attempt rows for the good events"
    );

    let again = run_consumer_for(&h, Duration::from_millis(500)).await;
    assert_eq!(
        again.processed, 0,
        "the next round claims nothing: not wedged"
    );
    assert_eq!(again.failed, 0);
    assert_eq!(h.count(UNPROCESSED, "none").await, 0);
    h.finish().await;
}
