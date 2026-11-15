//! `sanitize_database_name` — the guard between an operator's tenant name and
//! the contexts it lands in.
//!
//! The name is typed at a prompt and becomes a docker-compose project name and
//! a database name. Postgres identifiers cannot begin with a digit and cannot
//! be empty, and neither a compose project nor a `CREATE DATABASE` statement
//! should receive quotes, semicolons or whitespace from user input. So the
//! property asserted is a whitelist — what survives, not which characters are
//! rejected — because a blacklist tested case by case always misses one.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::cloud::tenant::create::sanitize_database_name;

fn is_safe(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
}

#[test]
fn an_ordinary_name_is_left_alone() {
    assert_eq!(sanitize_database_name("acme_prod"), "acme_prod");
    assert_eq!(sanitize_database_name("Tenant1"), "Tenant1");
}

// Why: this is the guard. Whatever an operator types, the result has to be
// usable as an identifier — so the assertion is on the output alphabet rather
// than on any particular character being caught.
#[test]
fn every_hostile_name_still_produces_a_usable_identifier() {
    for hostile in [
        "acme\"; DROP DATABASE postgres; --",
        "tenant name with spaces",
        "../../etc/passwd",
        "tenant$(whoami)",
        "tenant`id`",
        "tenant';--",
        "tenant\nnewline",
        "tenant\u{0}null",
        "'; CREATE ROLE evil SUPERUSER; --",
        "tenant--comment",
        "tenant/*block*/",
        "\u{1f600}emoji",
    ] {
        let sanitized = sanitize_database_name(hostile);
        assert!(
            is_safe(&sanitized),
            "{hostile:?} sanitised to {sanitized:?}, which is not a usable identifier"
        );
    }
}

// Why: an empty identifier is not valid in Postgres and an empty compose
// project name is not valid either. A name that sanitises to nothing needs a
// fallback rather than an empty string.
#[test]
fn a_name_with_nothing_usable_in_it_falls_back_rather_than_emptying() {
    for nothing_usable in ["", "---", "!!!", "   ", "///"] {
        let sanitized = sanitize_database_name(nothing_usable);
        assert!(
            is_safe(&sanitized),
            "{nothing_usable:?} sanitised to {sanitized:?}"
        );
    }

    assert_eq!(sanitize_database_name(""), "systemprompt");
}

// Why: Postgres rejects an unquoted identifier that starts with a digit, so a
// name like `2024tenant` has to be prefixed rather than passed through.
#[test]
fn a_name_starting_with_a_digit_is_prefixed() {
    assert_eq!(sanitize_database_name("2024tenant"), "db_2024tenant");
    assert_eq!(sanitize_database_name("9"), "db_9");

    assert!(
        is_safe(&sanitize_database_name("1")),
        "a single digit must still yield a usable identifier"
    );
}

// Why: sanitising twice must not keep changing the name. The result is used to
// derive further names, so an unstable transform would produce a different
// database on a second pass.
#[test]
fn sanitising_an_already_sanitised_name_changes_nothing() {
    for input in ["acme prod", "2024tenant", "", "a\"b"] {
        let once = sanitize_database_name(input);
        let twice = sanitize_database_name(&once);
        assert_eq!(once, twice, "sanitising {input:?} is not idempotent");
    }
}
