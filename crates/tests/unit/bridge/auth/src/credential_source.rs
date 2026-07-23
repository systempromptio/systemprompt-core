use std::collections::BTreeMap;
use systemprompt_bridge::auth::{has_credential_source, provider_chain};
use systemprompt_bridge::config::Config;
use systemprompt_bridge::integration::host_app::ProfileState;
use tempfile::TempDir;

fn config(toml: &str) -> Config {
    toml::from_str(toml).expect("config parses")
}

fn without_env<R>(f: impl FnOnce() -> R) -> R {
    temp_env::with_vars(vec![("SP_BRIDGE_PAT", None::<String>)], f)
}

#[test]
fn an_empty_config_has_no_credential_source() {
    assert!(!without_env(|| has_credential_source(&config(""))));
}

#[test]
fn an_inline_pat_env_var_counts_but_an_empty_one_does_not() {
    let set = temp_env::with_vars(vec![("SP_BRIDGE_PAT", Some("sp-live-x.y"))], || {
        has_credential_source(&config(""))
    });
    assert!(set, "a non-empty inline PAT is a credential source");

    let empty = temp_env::with_vars(vec![("SP_BRIDGE_PAT", Some(""))], || {
        has_credential_source(&config(""))
    });
    assert!(!empty, "an empty inline PAT is not a credential source");
}

#[test]
fn a_pat_file_counts_only_when_it_exists_on_disk() {
    let dir = TempDir::new().expect("tempdir");
    let missing = dir.path().join("absent.pat");
    assert!(!without_env(|| has_credential_source(&config(&format!(
        "[pat]\nfile = \"{}\"\n",
        missing.display()
    )))));

    let present = dir.path().join("present.pat");
    std::fs::write(&present, "sp-live-x.y").expect("write pat");
    assert!(without_env(|| has_credential_source(&config(&format!(
        "[pat]\nfile = \"{}\"\n",
        present.display()
    )))));
}

#[test]
fn a_tilde_pat_path_is_expanded_against_home() {
    let home = TempDir::new().expect("home");
    std::fs::write(home.path().join("bridge.pat"), "sp-live-x.y").expect("write pat");
    let found = temp_env::with_vars(
        vec![
            ("SP_BRIDGE_PAT", None),
            ("HOME", Some(home.path().display().to_string())),
        ],
        || has_credential_source(&config("[pat]\nfile = \"~/bridge.pat\"\n")),
    );
    assert!(found, "a `~/` PAT path resolves against HOME");
}

#[test]
fn an_enabled_session_section_is_a_credential_source_but_a_disabled_one_is_not() {
    assert!(without_env(|| has_credential_source(&config(
        "[session]\nenabled = true\n"
    ))));
    assert!(!without_env(|| has_credential_source(&config(
        "[session]\nenabled = false\n"
    ))));
}

#[test]
fn an_mtls_keystore_reference_is_a_credential_source() {
    assert!(without_env(|| has_credential_source(&config(
        "[mtls]\ncert_keystore_ref = \"device-cert\"\n"
    ))));
}

#[test]
fn the_provider_chain_runs_mtls_then_session_then_pat() {
    let names: Vec<&str> = provider_chain(&config(""))
        .iter()
        .map(|p| p.name())
        .collect();
    assert_eq!(
        names,
        vec!["mtls", "session", "pat"],
        "providers are ordered by descending registration priority"
    );
}

#[test]
fn profile_state_classifies_absent_partial_installed_and_stale() {
    let required = ["a", "b"];
    let empty = BTreeMap::new();
    assert!(matches!(
        ProfileState::classify(&required, &empty, None),
        ProfileState::Absent
    ));

    let mut partial = BTreeMap::new();
    partial.insert("a".to_owned(), "1".to_owned());
    match ProfileState::classify(&required, &partial, None) {
        ProfileState::Partial { missing_required } => {
            assert_eq!(missing_required, vec!["b".to_owned()]);
        },
        other => panic!("expected Partial, got {other:?}"),
    }

    let mut complete = partial.clone();
    complete.insert("b".to_owned(), "2".to_owned());
    let installed = ProfileState::classify(&required, &complete, Some(true));
    assert!(installed.is_installed(), "{installed:?}");

    assert!(
        matches!(
            ProfileState::classify(&required, &complete, Some(false)),
            ProfileState::Stale
        ),
        "a definite secret mismatch downgrades a complete profile to Stale"
    );
    assert!(
        ProfileState::classify(&required, &complete, None).is_installed(),
        "an unknown secret never downgrades"
    );
}
