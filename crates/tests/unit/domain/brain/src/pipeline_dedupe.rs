use systemprompt_brain::pipeline::dedupe::{
    DISPUTE_SIMILARITY, DedupeDecision, DedupeSnapshot, Existing, decide, forced_dispute,
};
use systemprompt_brain::pipeline::similarity::trigram_similarity;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::statement_hash;

use crate::fixtures::fact_id;

fn existing(id: &BrainFactId, statement: &str) -> Existing {
    Existing {
        id: id.clone(),
        statement: statement.to_owned(),
        statement_hash: statement_hash(statement),
    }
}

fn run(statement: &str, snapshot: &DedupeSnapshot) -> DedupeDecision {
    decide(statement, &statement_hash(statement), snapshot)
}

const STATEMENT: &str = "Acme owes billing a refund of the March invoice";

#[test]
fn a_tombstone_refuses_even_an_identity_hit() {
    let id = fact_id();
    let snapshot = DedupeSnapshot {
        tombstoned: true,
        by_identity: Some(existing(&id, STATEMENT)),
        neighbours: Vec::new(),
    };
    assert_eq!(run(STATEMENT, &snapshot), DedupeDecision::Tombstoned);
}

#[test]
fn an_identity_hit_with_the_same_statement_is_unchanged() {
    let id = fact_id();
    let snapshot = DedupeSnapshot {
        by_identity: Some(existing(&id, STATEMENT)),
        ..DedupeSnapshot::default()
    };
    assert_eq!(
        run(
            "  acme OWES billing a refund of the march invoice ",
            &snapshot
        ),
        DedupeDecision::Unchanged(id)
    );
}

#[test]
fn an_identity_hit_with_a_new_statement_revises() {
    let id = fact_id();
    let snapshot = DedupeSnapshot {
        by_identity: Some(existing(&id, STATEMENT)),
        ..DedupeSnapshot::default()
    };
    assert_eq!(
        run("Acme owes billing nothing", &snapshot),
        DedupeDecision::Revise(id)
    );
}

#[test]
fn the_same_statement_under_another_identity_merges() {
    let id = fact_id();
    let snapshot = DedupeSnapshot {
        neighbours: vec![existing(&id, STATEMENT)],
        ..DedupeSnapshot::default()
    };
    assert_eq!(run(STATEMENT, &snapshot), DedupeDecision::Merge(id));
}

#[test]
fn a_near_identical_statement_disputes_and_never_revises() {
    let id = fact_id();
    let other = fact_id();
    let snapshot = DedupeSnapshot {
        neighbours: vec![
            existing(&other, "Globex is a prospect"),
            existing(&id, STATEMENT),
        ],
        ..DedupeSnapshot::default()
    };
    let near = "Acme owes billing a refund of the March invoices";
    match run(near, &snapshot) {
        DedupeDecision::Dispute {
            fact,
            similarity: Some(score),
        } => {
            assert_eq!(fact, id);
            assert!(score > DISPUTE_SIMILARITY);
        },
        decision => panic!("expected a dispute, got {decision:?}"),
    }
}

#[test]
fn an_unrelated_statement_inserts() {
    let snapshot = DedupeSnapshot {
        neighbours: vec![existing(&fact_id(), STATEMENT)],
        ..DedupeSnapshot::default()
    };
    assert_eq!(
        run("Globex renewed for two years", &snapshot),
        DedupeDecision::Insert
    );
}

#[test]
fn a_validator_dispute_targets_the_identity_fact_only() {
    let id = fact_id();
    let hit = DedupeSnapshot {
        by_identity: Some(existing(&id, STATEMENT)),
        ..DedupeSnapshot::default()
    };
    assert_eq!(
        forced_dispute(&hit),
        Some(DedupeDecision::Dispute {
            fact: id,
            similarity: None
        })
    );
    assert_eq!(forced_dispute(&DedupeSnapshot::default()), None);
    let erased = DedupeSnapshot {
        tombstoned: true,
        ..DedupeSnapshot::default()
    };
    assert_eq!(forced_dispute(&erased), Some(DedupeDecision::Tombstoned));
}

#[test]
fn trigram_similarity_matches_pg_trgm() {
    assert!((trigram_similarity("word", "words") - 4.0 / 7.0).abs() < 1e-9);
    assert!((trigram_similarity("Same text", "same TEXT") - 1.0).abs() < 1e-9);
    assert!(trigram_similarity("alpha", "omega").abs() < 0.2);
    assert!(trigram_similarity("", "").abs() < f64::EPSILON);
}
