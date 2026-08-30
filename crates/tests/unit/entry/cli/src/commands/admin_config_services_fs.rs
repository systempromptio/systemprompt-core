//! `admin config services` — reading and setting the profile's port offset.
//!
//! `set` rewrites the whole profile file: it deserialises, mutates one field,
//! and serialises everything back. So the assertion that matters is not that
//! the offset changed but that nothing else did. A field the `Profile` type
//! fails to round-trip is silently dropped from the operator's profile on the
//! next `set`, and nothing reports it.
//!
//! These mutate the process-global bootstrap profile, which is safe only
//! because nextest runs one process per test — the file being rewritten is
//! this process's own temp profile.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt_cli::CliConfig;
use systemprompt_cli::admin::config::services::{ServicesCommands, SetArgs, execute};
use systemprompt_test_fixtures::ensure_test_bootstrap;

fn cfg() -> CliConfig {
    CliConfig::new().with_interactive(false)
}

fn profile_path() -> std::path::PathBuf {
    ensure_test_bootstrap().profile_path.clone()
}

fn profile_yaml() -> serde_yaml::Value {
    let raw = std::fs::read_to_string(profile_path()).expect("read profile");
    serde_yaml::from_str(&raw).expect("profile is valid yaml")
}

fn set_offset(offset: u16) -> anyhow::Result<()> {
    ensure_test_bootstrap();
    execute(
        &ServicesCommands::Set(SetArgs {
            port_offset: Some(offset),
        }),
        &cfg(),
    )
}

#[test]
fn showing_the_services_config_reads_without_touching_the_profile() {
    ensure_test_bootstrap();
    let before = std::fs::read_to_string(profile_path()).expect("read profile");

    execute(&ServicesCommands::Show, &cfg()).expect("show should succeed");

    assert_eq!(
        std::fs::read_to_string(profile_path()).expect("read profile"),
        before,
        "a read-only command must not rewrite the profile"
    );
}

// Why: `--port-offset` is the only option this command takes. Without it there
// is nothing to apply, and rewriting the profile anyway would be a write with
// no intent behind it.
#[test]
fn setting_nothing_is_refused_rather_than_rewriting_the_profile() {
    ensure_test_bootstrap();
    let before = std::fs::read_to_string(profile_path()).expect("read profile");

    let err = execute(
        &ServicesCommands::Set(SetArgs { port_offset: None }),
        &cfg(),
    )
    .expect_err("a set with no field to set must be refused");

    assert!(
        format!("{err:#}").contains("port-offset"),
        "the refusal should name the option: {err:#}"
    );
    assert_eq!(
        std::fs::read_to_string(profile_path()).expect("read profile"),
        before,
        "a refused set must leave the profile untouched"
    );
}

#[test]
fn setting_the_offset_persists_it_to_the_profile() {
    set_offset(7000).expect("set port offset");

    assert_eq!(
        profile_yaml()["services"]["port_offset"].as_u64(),
        Some(7000),
        "the offset an operator typed is the offset stored"
    );
}

/// Every key present before must still be present, with the same value.
/// Added keys are tolerated: the round trip materialises serde defaults that
/// the file previously omitted — `security.id_jag_ttl_secs` is written
/// explicitly afterwards with the default it already had. That changes the
/// file without changing the configuration, so the property to hold is that
/// nothing is lost or altered, not that the file is byte-identical.
fn assert_nothing_lost(before: &serde_yaml::Value, after: &serde_yaml::Value, path: &str) {
    match before {
        serde_yaml::Value::Mapping(before_map) => {
            let after_map = after
                .as_mapping()
                .unwrap_or_else(|| panic!("{path} stopped being a mapping"));
            for (key, value) in before_map {
                let child = after_map
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}.{key:?} was dropped by the rewrite"));
                assert_nothing_lost(value, child, &format!("{path}.{key:?}"));
            }
        },
        other => assert_eq!(other, after, "{path} changed value"),
    }
}

// Why: this is the whole risk of a command that rewrites the profile. Every
// other section must survive; a field the `Profile` type fails to round-trip
// is gone from the operator's configuration with no error.
#[test]
fn setting_the_offset_preserves_every_other_section_of_the_profile() {
    ensure_test_bootstrap();
    let before = profile_yaml();

    set_offset(8100).expect("set port offset");

    let after = profile_yaml();
    let before_map = before.as_mapping().expect("profile is a mapping");
    let after_map = after.as_mapping().expect("profile is a mapping");

    for (key, value) in before_map {
        if key.as_str() == Some("services") {
            continue;
        }
        let child = after_map
            .get(key)
            .unwrap_or_else(|| panic!("section {key:?} was dropped by the rewrite"));
        assert_nothing_lost(value, child, &format!("{key:?}"));
    }
}

// Why: the services section carries more than the offset. Rewriting one field
// must not discard its siblings.
#[test]
fn setting_the_offset_preserves_the_rest_of_the_services_section() {
    ensure_test_bootstrap();
    let before = profile_yaml();
    let before_services = before["services"].as_mapping().cloned();

    set_offset(8200).expect("set port offset");

    if let Some(before_services) = before_services {
        let after = profile_yaml();
        let after_services = after["services"]
            .as_mapping()
            .expect("services section survives");

        for (key, value) in &before_services {
            if key.as_str() == Some("port_offset") {
                continue;
            }
            assert_eq!(
                after_services.get(key),
                Some(value),
                "services.{key:?} changed while setting the offset"
            );
        }
    }
}

#[test]
fn the_last_offset_written_is_the_one_that_survives() {
    set_offset(9100).expect("first set");
    set_offset(9200).expect("second set");

    assert_eq!(
        profile_yaml()["services"]["port_offset"].as_u64(),
        Some(9200)
    );
}
