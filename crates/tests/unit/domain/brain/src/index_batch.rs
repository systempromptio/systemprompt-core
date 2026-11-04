//! The pure collectors behind the consumer round's set-based reindex: the
//! fact ids a round lands on are kept once each in landing order, and the
//! alias rows of a set fold to one row per `(subject_kind, alias_norm)`
//! with the last fact's spelling, subject and owner stamp winning (the
//! caller orders facts by `(updated_at, id)`), skipping what the alias
//! check would refuse.

use chrono::{DateTime, Duration, Utc};
use systemprompt_brain::index::{AliasOwner, AliasRows, MAX_ALIAS_NORM_LEN, ReindexBatch};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::SubjectKey;

use crate::fixtures::fact_id;

fn owner<'a>(subject: &'a SubjectKey, fact: &'a BrainFactId, at: DateTime<Utc>) -> AliasOwner<'a> {
    AliasOwner {
        subject,
        fact,
        updated_at: at,
    }
}

#[test]
fn a_batch_keeps_each_landed_fact_once_in_landing_order() {
    let (a, b, c) = (fact_id(), fact_id(), fact_id());
    let mut batch = ReindexBatch::default();
    assert!(batch.is_empty());
    batch.extend([a.clone(), b.clone()]);
    batch.extend(Vec::new());
    batch.extend([b.clone(), c.clone(), a.clone()]);
    assert_eq!(batch.ids(), &[a, b, c]);
    assert_eq!(batch.len(), 3);
    assert!(!batch.is_empty());
}

#[test]
fn alias_rows_fold_duplicates_with_the_last_fact_winning() {
    let acme = SubjectKey::new("client", "acme");
    let globex = SubjectKey::new("client", "globex");
    let person = SubjectKey::new("person", "jane");
    let (a, g, p) = (fact_id(), fact_id(), fact_id());
    let first = Utc::now();
    let later = first + Duration::seconds(1);
    let rows = AliasRows::collect([
        (
            owner(&acme, &a, first),
            vec!["acme".to_owned(), "Jane  Doe".to_owned()],
        ),
        (
            owner(&globex, &g, later),
            vec!["globex".to_owned(), "jane doe".to_owned()],
        ),
        (owner(&person, &p, later), vec!["Jane Doe".to_owned()]),
    ]);
    assert_eq!(rows.kinds, vec!["client", "client", "client", "person"]);
    assert_eq!(rows.subjects, vec!["acme", "globex", "globex", "jane"]);
    assert_eq!(
        rows.aliases,
        vec!["acme", "jane doe", "globex", "Jane Doe"],
        "the later fact's spelling replaces the earlier one in its slot"
    );
    assert_eq!(rows.norms, vec!["acme", "jane doe", "globex", "jane doe"]);
    assert_eq!(
        rows.owners,
        vec![a.to_string(), g.to_string(), g.to_string(), p.to_string()],
        "the slot carries the fact that won it"
    );
    assert_eq!(rows.stamps, vec![first, later, later, later]);
    assert_eq!(rows.len(), 4);
}

#[test]
fn alias_rows_skip_what_the_length_check_would_refuse() {
    let acme = SubjectKey::new("client", "acme");
    let a = fact_id();
    let long = "x".repeat(MAX_ALIAS_NORM_LEN + 1);
    let longest = "y".repeat(MAX_ALIAS_NORM_LEN);
    let rows = AliasRows::collect([(
        owner(&acme, &a, Utc::now()),
        vec!["   ".to_owned(), long, longest.clone(), "acme".to_owned()],
    )]);
    assert_eq!(rows.aliases, vec![longest, "acme".to_owned()]);
    assert!(AliasRows::collect(std::iter::empty()).is_empty());
}
