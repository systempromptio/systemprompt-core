//! The pure three-way verdict of an explicit apply (credential.md §3.1).

use std::collections::BTreeMap;

use systemprompt_provider_contracts::{
    ChangeSource, DeclaredEntities, DeclaredEntity, entity_hash,
};
use systemprompt_scheduler::services::{Verdict, file_wins_verdict, merge_plane, verdict};

#[test]
fn the_four_rows_of_the_table() {
    assert_eq!(verdict(Some("b"), Some("b"), Some("b")), Verdict::Unchanged);
    assert_eq!(verdict(Some("b"), Some("f"), Some("b")), Verdict::Apply);
    assert_eq!(verdict(Some("b"), Some("b"), Some("d")), Verdict::Drift);
    assert_eq!(verdict(Some("b"), Some("f"), Some("d")), Verdict::Conflict);
}

#[test]
fn both_sides_moving_to_the_same_value_converge() {
    assert_eq!(verdict(Some("b"), Some("x"), Some("x")), Verdict::Converged);
    assert_eq!(verdict(Some("b"), None, None), Verdict::Converged);
}

#[test]
fn absence_is_a_value_on_every_side() {
    assert_eq!(
        verdict(None, Some("f"), None),
        Verdict::Apply,
        "new in the file"
    );
    assert_eq!(
        verdict(None, None, Some("d")),
        Verdict::Drift,
        "created on the console"
    );
    assert_eq!(
        verdict(Some("b"), None, Some("b")),
        Verdict::Apply,
        "deleted from the file"
    );
    assert_eq!(
        verdict(Some("b"), Some("b"), None),
        Verdict::Drift,
        "deleted on the console"
    );
    assert_eq!(verdict(None, Some("f"), Some("d")), Verdict::Conflict);
}

fn declared(pairs: &[(&str, &str)]) -> DeclaredEntities {
    pairs
        .iter()
        .map(|(k, v)| {
            (
                (*k).to_owned(),
                DeclaredEntity::new((*v).to_owned(), ChangeSource::Code("f.yaml".to_owned())),
            )
        })
        .collect()
}

#[test]
fn adoption_takes_the_file_as_base_only_where_both_sides_exist() {
    let file = declared(&[("both", "1"), ("file_only", "2")]);
    let db = [("both", "console"), ("db_only", "x")]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();

    let merges = merge_plane(&file, &db, &BTreeMap::new(), true, false);
    let by: BTreeMap<&str, Verdict> = merges
        .iter()
        .map(|m| (m.entity.as_str(), m.verdict))
        .collect();

    assert_eq!(by["both"], Verdict::Drift);
    assert_eq!(by["file_only"], Verdict::Apply);
    assert_eq!(by["db_only"], Verdict::Drift);
    let both = merges.iter().find(|m| m.entity == "both").expect("both");
    assert!(both.adopted);
    assert_eq!(both.base_hash.as_deref(), Some(entity_hash("1").as_str()));
}

#[test]
fn without_adoption_a_missing_base_makes_disagreement_a_conflict() {
    let file = declared(&[("both", "1")]);
    let db = [("both".to_owned(), "console".to_owned())]
        .into_iter()
        .collect();

    let merges = merge_plane(&file, &db, &BTreeMap::new(), false, false);

    assert_eq!(merges[0].verdict, Verdict::Conflict);
    assert!(!merges[0].adopted);
}

#[test]
fn file_wins_applies_every_disagreement_and_never_reports_drift_or_conflict() {
    assert_eq!(
        file_wins_verdict(Some("b"), Some("b"), Some("d")),
        Verdict::Apply
    );
    assert_eq!(
        file_wins_verdict(Some("b"), Some("f"), Some("d")),
        Verdict::Apply
    );
    assert_eq!(file_wins_verdict(None, None, Some("d")), Verdict::Apply);
    assert_eq!(
        file_wins_verdict(Some("b"), Some("b"), Some("b")),
        Verdict::Unchanged
    );
    assert_eq!(
        file_wins_verdict(None, Some("x"), Some("x")),
        Verdict::Converged
    );
}

#[test]
fn a_file_wins_merge_never_adopts_and_deletes_database_only_rows() {
    let file = declared(&[("both", "1")]);
    let db = [("both", "console"), ("db_only", "x")]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();

    let merges = merge_plane(&file, &db, &BTreeMap::new(), true, true);

    assert!(
        merges
            .iter()
            .all(|m| m.verdict == Verdict::Apply && !m.adopted)
    );
    let db_only = merges
        .iter()
        .find(|m| m.entity == "db_only")
        .expect("db_only");
    assert!(db_only.file_hash.is_none());
}
