use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::integration::hermes::HERMES_HOST;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileRemoval};

fn with_hermes_home<R>(body: impl FnOnce(&Path) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir: PathBuf = temp.path().to_path_buf();
    let vars: Vec<(&str, Option<String>)> = vec![("HERMES_HOME", Some(dir.display().to_string()))];
    temp_env::with_vars(vars, || body(&dir))
}

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        api_key: "loopback-secret-value".to_owned(),
        models: vec!["gpt-5".to_owned(), "gpt-5-mini".to_owned()],
        organization_uuid: Some("00000000-0000-4000-8000-000000000009".to_owned()),
        headers: Default::default(),
    }
}

fn install(home: &Path) {
    let generated = HERMES_HOST
        .generate_profile(&inputs())
        .expect("profile generated");
    HERMES_HOST
        .install_profile(&generated.path)
        .expect("install merges into config.yaml");
    _ = fs::remove_file(&generated.path);
    assert!(
        home.join("config.yaml").is_file(),
        "install writes HERMES_HOME/config.yaml"
    );
}

fn read_config(home: &Path) -> String {
    fs::read_to_string(home.join("config.yaml")).expect("config.yaml readable")
}

fn read_env(home: &Path) -> String {
    fs::read_to_string(home.join(".env")).expect(".env readable")
}

#[test]
fn generating_a_profile_writes_yaml_carrying_the_loopback_endpoint_and_key_marker() {
    let generated = with_hermes_home(|_| {
        HERMES_HOST
            .generate_profile(&inputs())
            .expect("profile generated")
    });
    let body = fs::read_to_string(&generated.path).expect("generated profile readable");
    assert!(
        body.contains("base_url: http://127.0.0.1:48217/v1"),
        "the gateway origin gains the /v1 suffix: {body}"
    );
    assert!(body.contains("api_mode: chat_completions"), "{body}");
    assert!(body.contains("provider: systemprompt-gateway"), "{body}");
    assert!(
        body.contains("key_env: OPENAI_API_KEY"),
        "the named provider reads its secret from .env: {body}"
    );
    assert!(
        body.contains("default: gpt-5\n"),
        "the first negotiated model is selected: {body}"
    );
    assert!(
        !body.contains("gpt-5-mini"),
        "only the first model is written: {body}"
    );
    assert!(
        body.contains("_systemprompt_openai_api_key: loopback-secret-value"),
        "the API key rides along under the private marker: {body}"
    );
    assert_eq!(generated.bytes, body.len());
    assert_ne!(generated.payload_uuid, generated.profile_uuid);
    _ = fs::remove_file(&generated.path);
}

#[test]
fn installing_a_profile_merges_the_model_block_and_preserves_foreign_keys() {
    with_hermes_home(|home| {
        fs::write(
            home.join("config.yaml"),
            "top_level: kept\nmodel:\n  temperature: 0.2\nproviders:\n  systemprompt-gateway:\n    \
             base_url: https://stale.example/v1\n  mine:\n    base_url: https://mine.example/v1\n",
        )
        .expect("seed config");
        fs::write(home.join(".env"), "OTHER_SECRET=abc\n").expect("seed env");

        install(home);

        let merged = read_config(home);
        assert!(
            merged.contains("top_level: kept"),
            "a foreign top-level key survives: {merged}"
        );
        assert!(
            merged.contains("temperature: 0.2"),
            "a foreign key inside model survives: {merged}"
        );
        assert!(
            merged.contains("base_url: http://127.0.0.1:48217/v1"),
            "the bridge base_url is merged in: {merged}"
        );
        assert!(
            !merged.contains("stale.example"),
            "a stale bridge-owned value is replaced: {merged}"
        );
        assert!(merged.contains("api_mode: chat_completions"), "{merged}");
        assert!(merged.contains("default: gpt-5"), "{merged}");
        assert!(
            merged.contains("mine.example"),
            "a user's other named provider survives: {merged}"
        );
        assert!(
            !merged.contains("_systemprompt_openai_api_key")
                && !merged.contains("loopback-secret-value"),
            "the key marker never reaches config.yaml: {merged}"
        );

        let env = read_env(home);
        assert!(
            env.contains("OPENAI_API_KEY=loopback-secret-value"),
            "the key lands in .env: {env}"
        );
        assert!(
            env.contains("OTHER_SECRET=abc"),
            "other .env lines are preserved: {env}"
        );
    });
}

#[test]
fn a_second_install_leaves_config_yaml_byte_stable() {
    with_hermes_home(|home| {
        fs::write(home.join("config.yaml"), "top_level: kept\n").expect("seed config");
        install(home);
        let first = fs::read(home.join("config.yaml")).expect("first config");
        let first_env = read_env(home);

        install(home);
        let second = fs::read(home.join("config.yaml")).expect("second config");
        assert_eq!(
            first,
            second,
            "a repeat install must not rewrite config.yaml differently:\n{}",
            String::from_utf8_lossy(&second)
        );
        assert_eq!(
            first_env,
            read_env(home),
            "a repeat install must not duplicate the OPENAI_API_KEY line"
        );
    });
}

#[test]
fn removing_a_profile_strips_only_the_owned_keys_and_the_api_key_line() {
    with_hermes_home(|home| {
        fs::write(
            home.join("config.yaml"),
            "top_level: kept\nmodel:\n  temperature: 0.2\nproviders:\n  mine:\n    base_url: \
             https://mine.example/v1\n",
        )
        .expect("seed config");
        fs::write(home.join(".env"), "OTHER_SECRET=abc\n").expect("seed env");
        install(home);

        let removal = HERMES_HOST.remove_profile().expect("remove succeeds");
        assert!(
            matches!(removal, ProfileRemoval::Removed { .. }),
            "an installed profile is reported as removed, got {removal:?}"
        );

        let after = read_config(home);
        assert!(
            after.contains("top_level: kept"),
            "foreign top-level key survives removal: {after}"
        );
        assert!(
            after.contains("temperature: 0.2"),
            "foreign model key survives removal: {after}"
        );
        for owned in [
            "systemprompt-gateway",
            "api_mode",
            "key_env",
            "default: gpt-5",
        ] {
            assert!(
                !after.contains(owned),
                "owned key {owned} must be stripped: {after}"
            );
        }
        assert!(
            after.contains("mine.example"),
            "a user's other named provider survives removal: {after}"
        );

        let env = read_env(home);
        assert!(
            !env.contains("OPENAI_API_KEY"),
            "the API key line is removed: {env}"
        );
        assert!(
            env.contains("OTHER_SECRET=abc"),
            "other .env lines survive removal: {env}"
        );

        let again = HERMES_HOST
            .remove_profile()
            .expect("second remove succeeds");
        assert!(
            matches!(again, ProfileRemoval::NothingToRemove),
            "a second remove finds nothing, got {again:?}"
        );
    });
}

#[test]
fn removing_when_nothing_was_installed_reports_nothing_to_remove() {
    with_hermes_home(|home| {
        let removal = HERMES_HOST.remove_profile().expect("remove on empty home");
        assert!(
            matches!(removal, ProfileRemoval::NothingToRemove),
            "got {removal:?}"
        );
        assert!(
            !home.join("config.yaml").exists() && !home.join(".env").exists(),
            "a remove never creates files"
        );
    });
}
