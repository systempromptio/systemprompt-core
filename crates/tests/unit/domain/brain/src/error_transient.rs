//! Which brain failures are worth retrying: storage and the sinks are,
//! a row the schema's constraints refuse is not, since it fails the same
//! way every time and a retry could only repeat it.

use systemprompt_brain::BrainError;
use systemprompt_traits::{ConstraintKind, RepositoryError};

fn constraint(kind: ConstraintKind) -> RepositoryError {
    RepositoryError::Constraint {
        kind,
        constraint: "brain_subjects_subject_id_check".to_owned(),
        source: Box::new(std::io::Error::other("new row violates check constraint")),
    }
}

#[test]
fn a_constraint_violation_is_permanent() {
    for kind in [
        ConstraintKind::Check,
        ConstraintKind::NotNull,
        ConstraintKind::ForeignKey,
        ConstraintKind::Unique,
        ConstraintKind::Exclusion,
    ] {
        let error = BrainError::Repository(constraint(kind));
        assert!(!error.is_transient(), "{kind}: {error}");
        assert_eq!(error.stage(), "process");
    }
}

#[test]
fn other_storage_failures_stay_transient() {
    let down = BrainError::Repository(RepositoryError::database(std::io::Error::other("down")));
    assert!(down.is_transient());
    let missing = BrainError::Repository(RepositoryError::not_found("brain_fact", "m_1"));
    assert!(missing.is_transient());
    assert!(BrainError::BudgetExhausted.is_transient());
    assert!(!BrainError::Tombstoned.is_transient());
}
