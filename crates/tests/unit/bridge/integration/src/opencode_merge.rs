use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileRemoval};
use systemprompt_bridge::integration::opencode::OPENCODE_HOST;
use tempfile::TempDir;

struct Paths {
    user: PathBuf,
    managed: PathBuf,
    auth: PathBuf,
}

fn write_bridge_config(config_home: &Path, managed_dir: &Path) {
    let dir = config_home.join("systemprompt");
    std::fs::create_dir_all(&dir).expect("bridge config dir");
    std::fs::write(
        dir.join("systemprompt-bridge.toml"),
        format!("[opencode]\nmanaged_dir = '{}'\n", managed_dir.display()),
    )
    .expect("bridge config");
}

fn sandbox<R>(f: impl FnOnce(&Paths) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed_dir = root.path().join("managed");
    std::fs::create_dir_all(&managed_dir).expect("managed dir");
    let data = root.path().join("data");
    let user_dir = root.path().join("config").join("opencode");
    std::fs::create_dir_all(&user_dir).expect("user dir");
    let paths = Paths {
        user: user_dir.join("opencode.json"),
        managed: managed_dir.join("opencode.json"),
        auth: data.join("opencode").join("auth.json"),
    };
    write_bridge_config(&root.path().join("config"), &managed_dir);
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        (
            "XDG_CONFIG_HOME",
            Some(root.path().join("config").display().to_string()),
        ),
        ("XDG_DATA_HOME", Some(data.display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
    ];
    let out = temp_env::with_vars(vars, || f(&paths));
    drop(root);
    out
}

fn inputs(models: &[&str]) -> ProfileGenInputs {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "openai".to_owned());
    ProfileGenInputs {
        model_limits: Default::default(),
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        host_token: HostToken::new("loopback-secret-value"),
        models: models.iter().map(|m| (*m).to_owned()).collect(),
        default_model: None,
        organization_uuid: Some("org-abc".to_owned()),
        headers,
        mcp_servers: Some(Vec::new()),
    }
}

fn install(models: &[&str]) {
    let generated = OPENCODE_HOST
        .generate_profile(&inputs(models))
        .expect("generate");
    OPENCODE_HOST
        .install_profile(&generated.path)
        .expect("install");
    _ = std::fs::remove_file(&generated.path);
}

fn read(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("readable")).expect("json")
}

#[test]
fn installing_merges_the_provider_block_into_the_user_config_and_preserves_foreign_keys() {
    sandbox(|p| {
        std::fs::write(
            &p.user,
            r#"{ "$schema": "https://opencode.ai/config.json", "theme": "dark", "provider": { "ollama": { "npm": "@ai-sdk/openai-compatible" } }, "mcp": { "mine": { "type": "remote", "url": "https://example.com/mcp" } } }"#,
        )
        .expect("seed");
        install(&["claude-sonnet-5", "gpt-4.1"]);
        let doc = read(&p.user);
        assert_eq!(doc["theme"], "dark", "{doc}");
        assert_eq!(doc["$schema"], "https://opencode.ai/config.json");
        assert_eq!(
            doc["provider"]["ollama"]["npm"],
            "@ai-sdk/openai-compatible"
        );
        assert_eq!(doc["mcp"]["mine"]["url"], "https://example.com/mcp");
        let ours = &doc["provider"]["systemprompt"];
        assert_eq!(ours["options"]["baseURL"], "http://127.0.0.1:48217/v1");
        assert_eq!(
            ours["options"]["apiKey"], "loopback-secret-value",
            "OpenCode 2.x reads the key from the provider block: {doc}"
        );
        assert_eq!(ours["models"]["gpt-4.1"]["name"], "gpt-4.1");
        assert_eq!(doc["model"], "systemprompt/claude-sonnet-5");
        assert_eq!(
            doc["enabled_providers"],
            serde_json::json!(["systemprompt"]),
            "only the gateway provider may load: {doc}"
        );
        assert!(
            !p.managed.exists(),
            "the admin tier OpenCode 2.x ignores is never written"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p.user)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "the user config now holds the key");
        }
    });
}

#[test]
fn an_existing_allowlist_is_replaced_by_ours_and_only_ours_is_removed() {
    sandbox(|p| {
        std::fs::write(&p.user, r#"{ "enabled_providers": ["anthropic"] }"#).expect("seed");
        install(&["claude-sonnet-5"]);
        let doc = read(&p.user);
        assert_eq!(
            doc["enabled_providers"],
            serde_json::json!(["systemprompt"])
        );

        OPENCODE_HOST.remove_profile().expect("remove");
        assert!(!p.user.exists(), "nothing but our keys was left");

        std::fs::write(
            &p.user,
            r#"{ "enabled_providers": ["systemprompt", "anthropic"] }"#,
        )
        .expect("seed widened list");
        let removal = OPENCODE_HOST.remove_profile().expect("remove");
        assert!(
            matches!(removal, ProfileRemoval::NothingToRemove),
            "a list a user widened is not ours to delete: {removal:?}"
        );
    });
}

#[test]
fn install_drops_the_auth_json_key_opencode_no_longer_reads() {
    sandbox(|p| {
        std::fs::create_dir_all(p.auth.parent().expect("parent")).expect("data dir");
        std::fs::write(
            &p.auth,
            r#"{ "systemprompt": { "type": "api", "key": "stale" }, "anthropic": { "type": "oauth", "refresh": "r", "access": "a", "expires": 1 } }"#,
        )
        .expect("seed auth");
        install(&["claude-sonnet-5"]);
        let auth = read(&p.auth);
        assert!(auth.get("systemprompt").is_none(), "{auth}");
        assert_eq!(auth["anthropic"]["type"], "oauth", "{auth}");
    });
}

#[test]
fn a_non_object_user_config_is_never_overwritten() {
    sandbox(|p| {
        std::fs::write(&p.user, "[1, 2, 3]").expect("seed");
        let generated = OPENCODE_HOST
            .generate_profile(&inputs(&["claude-sonnet-5"]))
            .expect("generate");
        let err = OPENCODE_HOST
            .install_profile(&generated.path)
            .expect_err("a foreign config aborts the install");
        assert!(err.to_string().contains("refusing"), "{err}");
        assert_eq!(std::fs::read_to_string(&p.user).expect("config"), "[1, 2, 3]");
        _ = std::fs::remove_file(&generated.path);
    });
}

#[test]
fn a_shrunk_model_list_leaves_no_stale_entries() {
    sandbox(|p| {
        install(&["claude-sonnet-5", "gpt-4.1"]);
        install(&["gpt-4.1"]);
        let doc = read(&p.user);
        assert!(
            doc["provider"]["systemprompt"]["models"]
                .get("claude-sonnet-5")
                .is_none(),
            "{doc}"
        );
        assert_eq!(doc["model"], "systemprompt/gpt-4.1");
    });
}

#[test]
fn an_empty_model_list_drops_our_default_but_keeps_a_foreign_one() {
    sandbox(|p| {
        install(&["claude-sonnet-5"]);
        install(&[]);
        let doc = read(&p.user);
        assert!(doc.get("model").is_none(), "{doc}");

        std::fs::write(&p.user, r#"{ "model": "anthropic/claude-opus-5" }"#)
            .expect("seed foreign default");
        install(&[]);
        let doc = read(&p.user);
        assert_eq!(
            doc["model"], "anthropic/claude-opus-5",
            "a default naming another provider is not ours to remove"
        );
    });
}

#[test]
fn a_second_install_is_byte_stable() {
    sandbox(|p| {
        install(&["claude-sonnet-5"]);
        let first = std::fs::read(&p.user).expect("first");
        install(&["claude-sonnet-5"]);
        assert_eq!(std::fs::read(&p.user).expect("second"), first);
    });
}

#[test]
fn removing_strips_only_our_keys_and_reports_what_happened() {
    sandbox(|p| {
        std::fs::write(&p.user, r#"{ "theme": "dark" }"#).expect("seed");
        install(&["claude-sonnet-5"]);

        let removal = OPENCODE_HOST.remove_profile().expect("remove");
        assert!(
            matches!(removal, ProfileRemoval::Removed { .. }),
            "{removal:?}"
        );
        let doc = read(&p.user);
        assert_eq!(doc, serde_json::json!({ "theme": "dark" }), "{doc}");

        let again = OPENCODE_HOST.remove_profile().expect("second remove");
        assert!(
            matches!(again, ProfileRemoval::NothingToRemove),
            "{again:?}"
        );
    });
}

#[test]
fn removing_also_clears_a_retired_admin_tier_block() {
    sandbox(|p| {
        std::fs::write(
            &p.managed,
            r#"{ "theme": "dark", "provider": { "systemprompt": { "npm": "@ai-sdk/openai-compatible" } }, "model": "systemprompt/claude-sonnet-5" }"#,
        )
        .expect("seed retired block");
        install(&["claude-sonnet-5"]);
        assert!(
            systemprompt_bridge::integration::opencode::retired_admin_tier().is_some(),
            "doctor sees the retired block"
        );
        OPENCODE_HOST.remove_profile().expect("remove");
        assert_eq!(read(&p.managed), serde_json::json!({ "theme": "dark" }));
        assert!(!p.user.exists());
        assert!(systemprompt_bridge::integration::opencode::retired_admin_tier().is_none());
    });
}

#[test]
fn removing_when_nothing_was_installed_is_a_no_op() {
    sandbox(|_| {
        let removal = OPENCODE_HOST.remove_profile().expect("remove");
        assert!(
            matches!(removal, ProfileRemoval::NothingToRemove),
            "{removal:?}"
        );
    });
}
