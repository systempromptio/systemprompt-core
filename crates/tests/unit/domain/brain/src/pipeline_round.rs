//! The consumer round's cache: a lookup outside what the round preloaded
//! answers nothing, a covered identity answers the snapshot the store
//! would, and the writes an event commits change what the next event of
//! the same round dedupes against, so two events for one identity in one
//! round resolve to one fact revised rather than two facts. Once the
//! reads are forgotten every lookup answers nothing and applied writes
//! are dropped, while the ledger claims stay.

use std::collections::HashSet;

use chrono::Utc;
use systemprompt_brain::pipeline::dedupe::{DedupeDecision, decide};
use systemprompt_brain::pipeline::{CachedFact, DedupeWrite, RoundCache, RoundRows};
use systemprompt_identifiers::EventOutboxId;
use systemprompt_models::brain::{FactStatus, SubjectKey, statement_hash};

use crate::fixtures::{fact_id, kind, subject};

const HASH: &str = "identity-a";

fn covering(subject: &SubjectKey, hashes: &[&str]) -> RoundCache {
    let mut rows = RoundRows::default();
    rows.pairs.insert((kind("pricing"), subject.clone()));
    for hash in hashes {
        rows.identities
            .insert((kind("pricing"), (*hash).to_owned()));
    }
    RoundCache::new(rows)
}

fn created(subject: &SubjectKey, hash: &str, statement: &str) -> CachedFact {
    CachedFact {
        id: fact_id(),
        kind: kind("pricing"),
        subject: subject.clone(),
        identity_hash: hash.to_owned(),
        statement: statement.to_owned(),
        statement_hash: statement_hash(statement),
        live: true,
        updated_at: Utc::now(),
    }
}

#[test]
fn an_uncovered_pair_or_identity_answers_nothing() {
    let acme = subject("product:acme");
    let cache = covering(&acme, &[HASH]);
    assert!(
        cache
            .snapshot(&kind("pricing"), &subject("product:globex"), HASH)
            .is_none()
    );
    assert!(cache.snapshot(&kind("pricing"), &acme, "other").is_none());
    assert!(cache.snapshot(&kind("ownership"), &acme, HASH).is_none());
    assert!(!cache.claimed(&EventOutboxId::generate()));
    assert!(cache.resolved(&acme).is_none());
}

#[test]
fn a_covered_identity_with_no_rows_inserts_and_the_write_turns_the_next_into_a_revision() {
    let acme = subject("product:acme");
    let cache = covering(&acme, &[HASH]);
    let first = "acme costs 100";
    let snapshot = cache
        .snapshot(&kind("pricing"), &acme, HASH)
        .expect("covered");
    assert_eq!(
        decide(first, &statement_hash(first), &snapshot),
        DedupeDecision::Insert
    );

    let fact = created(&acme, HASH, first);
    cache.apply(vec![DedupeWrite::Created(fact.clone())], Utc::now());
    let second = "acme costs 120";
    let snapshot = cache
        .snapshot(&kind("pricing"), &acme, HASH)
        .expect("covered");
    assert_eq!(
        decide(second, &statement_hash(second), &snapshot),
        DedupeDecision::Revise(fact.id.clone()),
        "the same identity in the same round revises the fact the round created"
    );

    cache.apply(
        vec![DedupeWrite::Revised {
            id: fact.id.clone(),
            statement: second.to_owned(),
            statement_hash: statement_hash(second),
            status: FactStatus::Proposed,
        }],
        Utc::now(),
    );
    let snapshot = cache
        .snapshot(&kind("pricing"), &acme, HASH)
        .expect("covered");
    assert_eq!(
        decide(second, &statement_hash(second), &snapshot),
        DedupeDecision::Unchanged(fact.id),
        "a third event with the revised statement leaves it"
    );
}

#[test]
fn neighbours_follow_liveness_and_a_tombstone_refuses() {
    let acme = subject("product:acme");
    let other = "identity-b";
    let mut rows = RoundRows::default();
    rows.pairs.insert((kind("pricing"), acme.clone()));
    rows.identities.extend([
        (kind("pricing"), HASH.to_owned()),
        (kind("pricing"), other.to_owned()),
    ]);
    rows.tombstones.insert((kind("pricing"), other.to_owned()));
    rows.facts
        .push(created(&acme, "identity-c", "acme costs 100"));
    rows.claimed = HashSet::from([EventOutboxId::generate()]);
    let cache = RoundCache::new(rows);

    let snapshot = cache
        .snapshot(&kind("pricing"), &acme, other)
        .expect("covered");
    assert!(snapshot.tombstoned);

    let snapshot = cache
        .snapshot(&kind("pricing"), &acme, HASH)
        .expect("covered");
    assert_eq!(snapshot.neighbours.len(), 1, "{snapshot:?}");
    assert_eq!(
        decide(
            "acme costs 100",
            &statement_hash("acme costs 100"),
            &snapshot
        ),
        DedupeDecision::Merge(snapshot.neighbours[0].id.clone())
    );

    let staled = snapshot.neighbours[0].id.clone();
    cache.apply(vec![DedupeWrite::Staled(vec![staled])], Utc::now());
    let snapshot = cache
        .snapshot(&kind("pricing"), &acme, HASH)
        .expect("covered");
    assert!(
        snapshot.neighbours.is_empty(),
        "a staled fact is no longer a neighbour"
    );
}

#[test]
fn forgotten_reads_answer_nothing_and_keep_the_claims() {
    let acme = subject("product:acme");
    let cache = covering(&acme, &[HASH]);
    let claimed = EventOutboxId::generate();
    let mut rows = RoundRows::default();
    rows.claimed = HashSet::from([claimed.clone()]);
    rows.pairs.insert((kind("pricing"), acme.clone()));
    rows.identities.insert((kind("pricing"), HASH.to_owned()));
    rows.resolved
        .insert(SubjectKey::new("product", "ACME"), acme.clone());
    let cache_with_claim = RoundCache::new(rows);
    assert!(cache.reads());
    assert!(cache.snapshot(&kind("pricing"), &acme, HASH).is_some());

    cache.forget_reads();
    cache_with_claim.forget_reads();
    assert!(!cache.reads());
    assert!(cache.snapshot(&kind("pricing"), &acme, HASH).is_none());
    assert!(
        cache_with_claim
            .resolved(&SubjectKey::new("product", "ACME"))
            .is_none()
    );
    assert!(cache_with_claim.claimed(&claimed), "claims survive");

    let fact = created(&acme, HASH, "acme costs 1200 in emea");
    cache.apply(vec![DedupeWrite::Created(fact)], Utc::now());
    assert!(
        cache.snapshot(&kind("pricing"), &acme, HASH).is_none(),
        "a write applied after the reads were forgotten is dropped"
    );
}
