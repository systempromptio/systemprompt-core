use systemprompt_bridge::auth::{has_credential_source, provider_chain};
use systemprompt_bridge::config::Config;
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
fn a_file_store_counts_only_when_the_profile_pat_exists() {
    let home = TempDir::new().expect("home");
    let vars = vec![
        ("SP_BRIDGE_PAT", None),
        ("SP_BRIDGE_CONFIG", None),
        ("XDG_CONFIG_HOME", Some(home.path().display().to_string())),
    ];
    temp_env::with_vars(vars, || {
        let file_store = config("[pat]\nstore = 'file'\n");
        assert!(
            !has_credential_source(&file_store),
            "no PAT has been written yet"
        );
        let profile = systemprompt_bridge::config::gateways::active().expect("profile");
        std::fs::create_dir_all(&profile.dir).expect("profile dir");
        std::fs::write(profile.pat_file(), "sp-live-x.y").expect("write pat");
        assert!(has_credential_source(&file_store));
        assert!(
            !has_credential_source(&config("[pat]\n")),
            "no recorded store"
        );
    });
}

#[test]
fn a_keyring_store_counts_without_reading_the_secret() {
    assert!(without_env(|| has_credential_source(&config(
        "[pat]\nstore = 'keyring'\n"
    ))));
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
fn the_provider_chain_runs_session_then_pat() {
    let names: Vec<&str> = provider_chain(&config(""))
        .iter()
        .map(|p| p.name())
        .collect();
    assert_eq!(
        names,
        vec!["session", "pat"],
        "providers are ordered by descending registration priority"
    );
}
