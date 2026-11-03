//! A subject key is bounded to what `brain_subjects` admits: a kind of 1
//! to 64 characters and an id of 1 to 255, counted in characters as
//! Postgres counts them, whether it is parsed from `<kind>:<id>` or
//! built from its halves.

use systemprompt_models::brain::{
    MAX_SUBJECT_ID_LEN, MAX_SUBJECT_KIND_LEN, SubjectKey, SubjectKeyError,
};

#[test]
fn a_subject_within_both_bounds_is_admitted() {
    let key: SubjectKey = "client:acme".parse().expect("parses");
    assert_eq!(key, SubjectKey::new("client", "acme"));
    assert_eq!(key.validate(), Ok(()));
    let longest = SubjectKey::try_new(
        "k".repeat(MAX_SUBJECT_KIND_LEN),
        "i".repeat(MAX_SUBJECT_ID_LEN),
    );
    assert!(longest.is_ok(), "{longest:?}");
}

#[test]
fn an_oversized_id_is_refused_wherever_it_enters() {
    let id = "x".repeat(300);
    assert_eq!(
        SubjectKey::new("client", &id).validate(),
        Err(SubjectKeyError::IdLength { len: 300 })
    );
    assert_eq!(
        SubjectKey::try_new("client", &id),
        Err(SubjectKeyError::IdLength { len: 300 })
    );
    assert_eq!(
        format!("client:{id}").parse::<SubjectKey>(),
        Err(SubjectKeyError::IdLength { len: 300 })
    );
}

#[test]
fn an_oversized_kind_is_refused_before_the_id_is_looked_at() {
    let kind = "k".repeat(MAX_SUBJECT_KIND_LEN + 1);
    assert_eq!(
        SubjectKey::new(&kind, "x".repeat(300)).validate(),
        Err(SubjectKeyError::KindLength { len: 65 })
    );
}

#[test]
fn bounds_count_characters_not_bytes() {
    let id = "é".repeat(MAX_SUBJECT_ID_LEN);
    assert!(id.len() > MAX_SUBJECT_ID_LEN, "a multibyte id");
    assert_eq!(SubjectKey::new("client", id).validate(), Ok(()));
}

#[test]
fn a_string_without_both_halves_is_a_shape_error() {
    for bad in ["client", ":acme", "client:", ""] {
        assert_eq!(
            bad.parse::<SubjectKey>(),
            Err(SubjectKeyError::Shape(bad.to_owned())),
            "{bad:?}"
        );
    }
}
