//! The schema phase's retry classifier reads the SQLSTATE: a lock timeout, a
//! deadlock and a serialization failure are retried, any other code is not,
//! even when its message mentions a lock, and only an error that carries no
//! code falls back to the message.

use systemprompt_database::{
    RETRYABLE_SQLSTATES, is_retryable_lock_conflict, is_retryable_sqlstate,
};
use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::RepositoryError;

fn database_error(sqlstate: Option<&str>, message: &str) -> RepositoryError {
    RepositoryError::Database {
        sqlstate: sqlstate.map(str::to_owned),
        source: Box::new(std::io::Error::other(message.to_owned())),
    }
}

fn phase_failure(source: RepositoryError) -> LoaderError {
    LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("ext"),
        context: "Statement 2/3 failed".to_owned(),
        source: Box::new(source),
    }
}

#[test]
fn the_three_lock_conflict_sqlstates_are_retryable() {
    assert_eq!(RETRYABLE_SQLSTATES, ["55P03", "40P01", "40001"]);
    for code in RETRYABLE_SQLSTATES {
        assert!(is_retryable_sqlstate(code), "{code}");
        let error = phase_failure(database_error(Some(code), "server message"));
        assert!(is_retryable_lock_conflict(&error), "{code}");
    }
}

#[test]
fn a_deadlock_is_retried() {
    let error = phase_failure(database_error(Some("40P01"), "deadlock detected"));
    assert!(is_retryable_lock_conflict(&error));
}

#[test]
fn another_sqlstate_is_not_retried_whatever_its_message_says() {
    for code in ["42P01", "23505", "42601", "57014"] {
        assert!(!is_retryable_sqlstate(code), "{code}");
        let error = phase_failure(database_error(
            Some(code),
            "deadlock detected on lock timeout",
        ));
        assert!(!is_retryable_lock_conflict(&error), "{code}");
    }
}

#[test]
fn an_error_without_a_code_falls_back_to_the_message() {
    for message in [
        "canceling statement due to lock timeout",
        "deadlock detected",
        "could not serialize access due to concurrent update",
    ] {
        let error = phase_failure(database_error(None, message));
        assert!(is_retryable_lock_conflict(&error), "{message}");
    }
    let error = phase_failure(database_error(None, "relation \"x\" does not exist"));
    assert!(!is_retryable_lock_conflict(&error));
}
