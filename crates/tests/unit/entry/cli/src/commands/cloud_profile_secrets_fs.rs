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

// The other half of protecting the secrets file: `save_secrets` writes it 0600
// on disk, and `.dockerignore` is what keeps it out of the image built beside
// it. Either alone is insufficient.
mod build_context {
    use systemprompt_cli::cloud::profile::templates::{save_dockerignore, save_entrypoint};

    fn dockerignore() -> String {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ctx").join(".dockerignore");
        save_dockerignore(&path).expect("write dockerignore");
        std::fs::read_to_string(&path).expect("read dockerignore")
    }

    // Why: everything `save_secrets` protects on disk is copied into the image
    // unless it is excluded here. A dropped line ships live credentials inside
    // a distributed artefact, where file mode no longer helps.
    #[test]
    fn the_build_context_excludes_every_file_that_holds_credentials() {
        let content = dockerignore();

        for secret_path in [
            ".systemprompt/credentials.json",
            ".systemprompt/tenants.json",
            ".systemprompt/**/secrets.json",
            ".env*",
        ] {
            assert!(
                content.lines().any(|line| line.trim() == secret_path),
                "{secret_path} is not excluded; it would be copied into the image"
            );
        }
    }

    // Why: the repository history is not part of the application. Shipped, it
    // carries every secret ever committed and every branch name, in an image
    // that may be pushed to a registry.
    #[test]
    fn the_build_context_excludes_the_git_directory() {
        assert!(
            dockerignore().lines().any(|line| line.trim() == ".git"),
            "git history would ship inside the image"
        );
    }

    fn entrypoint() -> (String, std::path::PathBuf, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nested").join("entrypoint.sh");
        save_entrypoint(&path).expect("write entrypoint");
        let body = std::fs::read_to_string(&path).expect("read entrypoint");
        (body, path, dir)
    }

    // Why: the container runs this file directly. Written without the execute
    // bit it fails at start-up with a permission error rather than anything
    // naming the cause.
    #[cfg(unix)]
    #[test]
    fn the_entrypoint_is_executable() {
        use std::os::unix::fs::PermissionsExt;

        let (_body, path, _dir) = entrypoint();
        let mode = std::fs::metadata(&path)
            .expect("stat entrypoint")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(
            mode, 0o755,
            "an entrypoint without the execute bit cannot start the container"
        );
    }

    // Why: without `set -e` a failing command inside the entrypoint is ignored
    // and the script continues, so a broken container reports success until
    // something downstream notices.
    #[test]
    fn the_entrypoint_aborts_on_the_first_failure_and_replaces_its_shell() {
        let (body, _path, _dir) = entrypoint();

        assert!(
            body.lines().any(|line| line.trim() == "set -e"),
            "without `set -e` a failed command does not stop the entrypoint: {body}"
        );
        assert!(
            body.contains("exec "),
            "the server must replace the shell so it receives signals directly: {body}"
        );
        assert!(
            body.starts_with("#!"),
            "an entrypoint without a shebang is not directly executable: {body}"
        );
    }
}

// `existing_geoip_database` reads the profile a `cloud profile create` is about
// to overwrite, so the operator's configured GeoIP path survives a regenerate.
// The failure direction that matters is carrying a value forward from a file
// that did not parse: the regenerated profile would then claim a database the
// operator never configured in the form the new profile expects.
mod geoip_carry_forward {
    use std::path::Path;
    use systemprompt_cli::cloud::profile::templates::existing_geoip_database;
    use systemprompt_test_fixtures::ensure_test_bootstrap;

    /// Profiles are built from the bootstrap fixture's own file rather than
    /// hand-written YAML: `Profile` denies unknown fields and resolves paths
    /// relative to the profile, so a hand-rolled minimal document would test
    /// the parser's tolerance rather than this function.
    fn profile_with(dir: &Path, mutate: impl FnOnce(&mut serde_yaml::Value)) -> std::path::PathBuf {
        let source = &ensure_test_bootstrap().profile_path;
        let raw = std::fs::read_to_string(source).expect("read bootstrap profile");
        let mut doc: serde_yaml::Value = serde_yaml::from_str(&raw).expect("parse profile");
        mutate(&mut doc);

        let path = dir.join("profile.yaml");
        std::fs::write(
            &path,
            serde_yaml::to_string(&doc).expect("serialise profile"),
        )
        .expect("write profile");
        path
    }

    #[tokio::test]
    async fn a_configured_geoip_path_is_carried_forward() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = profile_with(dir.path(), |doc| {
            doc["paths"]["geoip_database"] =
                serde_yaml::Value::String("/app/geoip/GeoLite2-City.mmdb".to_owned());
        });

        assert!(
            existing_geoip_database(&path).is_some(),
            "a configured GeoIP database must survive a profile regenerate"
        );
    }

    #[tokio::test]
    async fn a_profile_without_a_geoip_path_carries_nothing_forward() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = profile_with(dir.path(), |doc| {
            if let Some(paths) = doc["paths"].as_mapping_mut() {
                paths.remove(serde_yaml::Value::String("geoip_database".to_owned()));
            }
        });

        assert!(existing_geoip_database(&path).is_none());
    }

    // Why: this is the guard. A profile that no longer parses tells us nothing
    // about what was configured, so nothing may be carried into the new one.
    #[tokio::test]
    async fn an_unparseable_profile_carries_nothing_forward() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("profile.yaml");
        std::fs::write(&path, "this: [is not: a profile").expect("write broken profile");

        assert!(
            existing_geoip_database(&path).is_none(),
            "an unreadable profile must not be mined for values"
        );
    }

    #[tokio::test]
    async fn a_missing_profile_carries_nothing_forward() {
        let dir = tempfile::tempdir().expect("tempdir");

        assert!(existing_geoip_database(&dir.path().join("absent.yaml")).is_none());
    }

    // Why: a path outside the container root cannot exist in the image. It is
    // still carried forward — the operator may be mounting it — but the
    // decision is to warn rather than silently drop, and dropping it would
    // disable GeoIP without saying so.
    #[tokio::test]
    async fn a_geoip_path_outside_the_container_root_is_still_carried_forward() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = profile_with(dir.path(), |doc| {
            doc["paths"]["geoip_database"] =
                serde_yaml::Value::String("/opt/elsewhere/GeoLite2-City.mmdb".to_owned());
        });

        assert!(
            existing_geoip_database(&path).is_some(),
            "an unusual path is warned about, not silently discarded"
        );
    }
}
