use systemprompt_brain::replay::{FactState, ReplayChange, ReplayReport, classify, tally};
use systemprompt_models::brain::FactStatus;

fn fact(kind: &str, identity: &str, statement: &str, status: FactStatus) -> FactState {
    FactState {
        kind: crate::fixtures::kind(kind),
        identity_hash: identity.to_owned(),
        statement_hash: statement.to_owned(),
        status,
    }
}

#[test]
fn a_fact_with_no_stored_twin_is_created() {
    let replayed = fact("k", "i", "s", FactStatus::Candidate);
    assert_eq!(classify(&replayed, None), ReplayChange::Create);
}

#[test]
fn a_changed_statement_is_a_revision() {
    let replayed = fact("k", "i", "s2", FactStatus::Confirmed);
    let stored = fact("k", "i", "s1", FactStatus::Confirmed);
    assert_eq!(classify(&replayed, Some(&stored)), ReplayChange::Revise);
}

#[test]
fn becoming_disputed_is_a_dispute_even_with_a_new_statement() {
    let replayed = fact("k", "i", "s2", FactStatus::Disputed);
    let stored = fact("k", "i", "s1", FactStatus::Confirmed);
    assert_eq!(classify(&replayed, Some(&stored)), ReplayChange::Dispute);
}

#[test]
fn identical_state_is_unchanged() {
    let replayed = fact("k", "i", "s", FactStatus::Confirmed);
    assert_eq!(
        classify(&replayed, Some(&replayed.clone())),
        ReplayChange::Unchanged
    );
}

#[test]
fn tally_matches_by_kind_and_identity_and_counts_per_kind() {
    let replayed = vec![
        fact("a", "1", "s", FactStatus::Candidate),
        fact("a", "2", "new", FactStatus::Confirmed),
        fact("b", "1", "s", FactStatus::Confirmed),
    ];
    let stored = vec![
        fact("a", "2", "old", FactStatus::Confirmed),
        fact("b", "1", "s", FactStatus::Confirmed),
        fact("c", "1", "s", FactStatus::Confirmed),
    ];
    let mut report = ReplayReport::default();
    tally(&mut report, &replayed, &stored);
    assert_eq!(
        (
            report.would_create,
            report.would_revise,
            report.would_dispute,
            report.unchanged
        ),
        (1, 1, 0, 1)
    );
    let a = report.per_kind[&crate::fixtures::kind("a")];
    assert_eq!((a.create, a.revise), (1, 1));
    assert_eq!(report.per_kind[&crate::fixtures::kind("b")].unchanged, 1);
    assert!(!report.per_kind.contains_key(&crate::fixtures::kind("c")));
}
