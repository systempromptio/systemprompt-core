use systemprompt_brain::{FactChange, NewFact};
use systemprompt_models::brain::{FactStatus, RevisionChange, statement_hash};

use crate::fixtures::{at, document_source, fact_id, new_fact, topic, user};

fn every_change() -> Vec<(FactChange, RevisionChange, Option<FactStatus>)> {
    vec![
        (
            FactChange::Revised {
                statement: "Billing is owned by ops".into(),
                fields: serde_json::Map::new(),
                source: document_source("doc-1", "rev-2"),
                topics: vec![topic("ops")],
                status: FactStatus::Proposed,
                importance_base: 0.4,
                review_after: Some(at(10)),
                actor: None,
            },
            RevisionChange::Revised,
            Some(FactStatus::Proposed),
        ),
        (
            FactChange::Merged {
                topics: vec![topic("ops")],
                actor: None,
            },
            RevisionChange::Merged,
            None,
        ),
        (
            FactChange::FieldsEdited {
                statement: "edited".into(),
                fields: serde_json::Map::new(),
                actor: user("u1"),
            },
            RevisionChange::FieldsEdited,
            None,
        ),
        (
            FactChange::Confirmed { by: user("u1") },
            RevisionChange::Confirmed,
            Some(FactStatus::Confirmed),
        ),
        (
            FactChange::Disputed { actor: None },
            RevisionChange::Disputed,
            Some(FactStatus::Disputed),
        ),
        (
            FactChange::DisputeResolved {
                status: FactStatus::Confirmed,
                actor: user("u1"),
            },
            RevisionChange::DisputeResolved,
            Some(FactStatus::Confirmed),
        ),
        (
            FactChange::Staled,
            RevisionChange::Staled,
            Some(FactStatus::Stale),
        ),
        (
            FactChange::Superseded { actor: None },
            RevisionChange::Superseded,
            Some(FactStatus::Superseded),
        ),
        (
            FactChange::Retracted { actor: user("u1") },
            RevisionChange::Retracted,
            Some(FactStatus::Retired),
        ),
        (
            FactChange::Pinned { actor: user("u1") },
            RevisionChange::Pinned,
            None,
        ),
        (
            FactChange::Unpinned { actor: user("u1") },
            RevisionChange::Unpinned,
            None,
        ),
        (FactChange::ReviewDue, RevisionChange::ReviewDue, None),
    ]
}

#[test]
fn every_change_records_its_own_revision_kind() {
    for (change, expected, _) in every_change() {
        assert_eq!(change.revision_change(), expected, "{change:?}");
    }
}

#[test]
fn every_revision_kind_but_created_has_a_change() {
    let mut recorded: Vec<&str> = every_change()
        .iter()
        .map(|(change, _, _)| change.revision_change().as_str())
        .collect();
    recorded.sort_unstable();
    recorded.dedup();
    assert_eq!(recorded.len(), 12);
    assert!(!recorded.contains(&RevisionChange::Created.as_str()));
}

#[test]
fn lifecycle_changes_set_the_expected_status() {
    for (change, _, status) in every_change() {
        assert_eq!(change.new_status(), status, "{change:?}");
    }
}

#[test]
fn pin_changes_set_pinned_and_nothing_else_does() {
    for (change, _, _) in every_change() {
        let expected = match change {
            FactChange::Pinned { .. } => Some(true),
            FactChange::Unpinned { .. } => Some(false),
            _ => None,
        };
        assert_eq!(change.new_pinned(), expected, "{change:?}");
    }
}

#[test]
fn confirmation_names_the_confirming_user() {
    let change = FactChange::Confirmed { by: user("u9") };
    assert_eq!(change.actor(), Some(&user("u9")));
    assert_eq!(FactChange::Staled.actor(), None);
}

#[test]
fn new_fact_mints_an_id_and_hashes_its_statement() {
    let fact: NewFact = new_fact("ownership", "service:billing", "Billing  is owned by OPS");
    assert!(fact.id.as_str().starts_with("m_"));
    assert_ne!(fact.id, fact_id());
    assert_eq!(
        fact.statement_hash,
        statement_hash("billing is owned by ops")
    );
    assert_eq!(fact.status, FactStatus::Proposed);
}
