//! `save_secrets` — the secrets file a cloud profile writes to disk.
//!
//! This file holds database credentials, provider API keys and the OAuth
//! at-rest pepper. Two properties matter more than its contents: it must not
//! be readable by anyone but its owner, and the pepper must be fresh per
//! profile — a constant one would mean every deployment shares the value that
//! makes at-rest hashes unguessable.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::Path;

use systemprompt_cli::cloud::profile::api_keys::ApiKeys;
use systemprompt_cli::cloud::profile::templates::{DatabaseUrls, save_secrets};

fn keys() -> ApiKeys {
    ApiKeys {
        gemini: Some("gem-key".to_owned()),
        anthropic: Some("ant-key".to_owned()),
        openai: None,
    }
}

fn write(dir: &Path, internal: Option<&str>) -> serde_json::Value {
    let path = dir.join("nested").join("secrets.json");
    save_secrets(
        &DatabaseUrls {
            external: "postgres://user:pw@host/db",
            internal,
        },
        &keys(),
        &path,
        false,
    )
    .expect("writing secrets should succeed");

    serde_json::from_str(&std::fs::read_to_string(&path).expect("read secrets"))
        .expect("secrets must be valid JSON")
}

// Why: the file holds live database credentials and API keys. Group- or
// world-readable, it hands them to every account on the host.
#[cfg(unix)]
#[test]
fn the_secrets_file_is_readable_only_by_its_owner() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("secrets.json");
    save_secrets(
        &DatabaseUrls {
            external: "postgres://user:pw@host/db",
            internal: None,
        },
        &keys(),
        &path,
        false,
    )
    .expect("write secrets");

    let mode = std::fs::metadata(&path)
        .expect("stat secrets")
        .permissions()
        .mode()
        & 0o777;

    assert_eq!(
        mode, 0o600,
        "secrets must be owner-only; {mode:o} exposes them to other accounts"
    );
}

// Why: the pepper is what makes at-rest hashes unguessable. Generated once and
// reused across profiles, one leaked value would compromise every deployment
// that shares it.
#[test]
fn each_profile_gets_a_fresh_at_rest_pepper() {
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");

    let a = write(first.path(), None);
    let b = write(second.path(), None);

    let pepper_a = a["oauth_at_rest_pepper"].as_str().expect("pepper");
    let pepper_b = b["oauth_at_rest_pepper"].as_str().expect("pepper");

    assert_ne!(
        pepper_a, pepper_b,
        "two profiles must not share an at-rest pepper"
    );
    assert!(
        pepper_a.len() >= 32,
        "a short pepper is a guessable one: {} chars",
        pepper_a.len()
    );
}

#[test]
fn the_api_keys_land_under_their_own_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let secrets = write(dir.path(), None);

    assert_eq!(secrets["gemini"].as_str(), Some("gem-key"));
    assert_eq!(secrets["anthropic"].as_str(), Some("ant-key"));
    assert!(
        secrets["openai"].is_null(),
        "a key that was not supplied must be null rather than an empty string a \
         provider would try to authenticate with"
    );
}

// Why: an absent internal URL must be absent, not present-and-empty. A boot
// that finds the field and connects to nothing fails later and further away
// than one that finds no field at all.
#[test]
fn the_internal_database_url_is_written_only_when_supplied() {
    let with = tempfile::tempdir().expect("tempdir");
    let without = tempfile::tempdir().expect("tempdir");

    let present = write(with.path(), Some("postgres://internal/db"));
    let absent = write(without.path(), None);

    assert_eq!(
        present["internal_database_url"].as_str(),
        Some("postgres://internal/db")
    );
    assert!(
        absent.get("internal_database_url").is_none(),
        "the field must be omitted entirely when there is no internal URL"
    );
}

#[test]
fn the_external_url_is_written_under_both_names_it_is_read_by() {
    let dir = tempfile::tempdir().expect("tempdir");
    let secrets = write(dir.path(), None);

    assert_eq!(
        secrets["database_url"].as_str(),
        secrets["external_database_url"].as_str(),
        "both keys name the same external database"
    );
}

// Why: the path is supplied by the caller and its directory may not exist yet.
// Failing here would abort profile creation partway, leaving a profile with no
// secrets beside it.
#[test]
fn a_missing_parent_directory_is_created_rather_than_failing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a").join("b").join("secrets.json");

    save_secrets(
        &DatabaseUrls {
            external: "postgres://user:pw@host/db",
            internal: None,
        },
        &keys(),
        &path,
        false,
    )
    .expect("a missing parent must be created");

    assert!(path.exists());
}
