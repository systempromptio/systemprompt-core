//! The refusal a node prints when it boots without migrating against a
//! database that is behind: it must list the work and name the command that
//! fixes it, for the profile that was booted.

use systemprompt_runtime::RuntimeError;

#[test]
fn the_refusal_lists_the_work_and_names_the_migrate_command() {
    let error = RuntimeError::SchemaBehind {
        profile: "prod-gke".to_owned(),
        fresh: vec!["marketplace".to_owned()],
        pending: vec!["users:042 add_flags".to_owned(), "ai:017 index".to_owned()],
        drift: vec![],
    };
    let message = error.to_string();

    assert!(
        message.contains("1 extension(s) not installed [marketplace]"),
        "{message}"
    );
    assert!(
        message.contains("2 pending migration(s) [users:042 add_flags, ai:017 index]"),
        "{message}"
    );
    assert!(message.contains("0 checksum drift(s)"), "{message}");
    assert!(
        message.contains("systemprompt infra db migrate --profile prod-gke"),
        "{message}"
    );
}
