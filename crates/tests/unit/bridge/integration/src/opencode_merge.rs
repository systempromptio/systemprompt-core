use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileRemoval};
use systemprompt_bridge::integration::opencode::OPENCODE_CLI_HOST as OPENCODE_HOST;
use tempfile::TempDir;

struct Paths {
    user: PathBuf,
    managed: PathBuf,
    auth: PathBuf,
}

fn write_bridge_config(config_home: &Path, managed_dir: &Path) {
    let dir = config_home
        .join("systemprompt")
        .join("gateways")
        .join("default");
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
        model_names: Default::default(),
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
        .install_profile(
            &generated.path,
            &systemprompt_bridge::elevation_gate::ElevationGate::new(),
        )
        .expect("install");
    _ = std::fs::remove_file(&generated.path);
}

fn read(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("readable")).expect("json")
}
fn token_file(p: &Paths) -> PathBuf {
    p.user.with_file_name("systemprompt.token")
}

#[test]
fn installing_writes_the_v2_provider_block_and_preserves_foreign_keys() {
    sandbox(|p| {
        std::fs::write(
            &p.user,
            r#"{ "$schema": "https://opencode.ai/config.json", "theme": "dark", "providers": { "ollama": { "package": "x" } }, "mcp": { "servers": { "mine": { "type": "remote", "url": "https://example.com/mcp" } } } }"#,
        )
        .expect("seed");
        install(&["claude-sonnet-5", "gpt-4.1"]);
        let doc = read(&p.user);
        assert_eq!(doc["theme"], "dark", "{doc}");
        assert_eq!(doc["providers"]["ollama"]["package"], "x");
        assert_eq!(
            doc["mcp"]["servers"]["mine"]["url"],
            "https://example.com/mcp"
        );
        let ours = &doc["providers"]["systemprompt"];
        assert_eq!(ours["settings"]["baseURL"], "http://127.0.0.1:48217/v1");
        assert_eq!(
            ours["settings"]["apiKey"],
            format!("{{file:{}}}", token_file(p).display()),
            "the key is named, never inlined: {doc}"
        );
        assert_eq!(ours["models"]["gpt-4.1"]["modelID"], "gpt-4.1");
        assert_eq!(doc["model"], "systemprompt/claude-sonnet-5");
        assert_eq!(
            doc["policies"],
            serde_json::json!([
                {"effect": "deny", "action": "provider.use", "resource": "*"},
                {"effect": "allow", "action": "provider.use", "resource": "systemprompt"},
            ])
        );
        assert!(doc.get("provider").is_none() && doc.get("enabled_providers").is_none());
        assert_eq!(
            std::fs::read_to_string(token_file(p)).expect("token"),
            "loopback-secret-value"
        );
        assert!(!p.managed.exists(), "the admin tier is never written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(token_file(p))
                .expect("meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "the token file is private");
        }
    });
}

#[test]
fn the_first_v2_install_strips_the_v1_keys_the_bridge_owned() {
    sandbox(|p| {
        std::fs::write(
            &p.user,
            r#"{ "theme": "dark", "provider": { "systemprompt": { "npm": "@ai-sdk/openai-compatible" }, "ollama": {} }, "enabled_providers": ["systemprompt"], "mcp": { "gateway-tools": { "type": "remote" }, "mine": { "type": "remote" } } }"#,
        )
        .expect("seed v1");
        std::fs::write(
            p.user.with_file_name(".systemprompt-mcp.json"),
            r#"{ "slugs": ["gateway-tools"] }"#,
        )
        .expect("seed sidecar");
        let v1_plugin = p
            .user
            .with_file_name("plugin")
            .join("systemprompt-hooks.js");
        std::fs::create_dir_all(v1_plugin.parent().expect("parent")).expect("plugin dir");
        std::fs::write(&v1_plugin, "export default {}").expect("seed v1 plugin");
        install(&["claude-sonnet-5"]);
        let doc = read(&p.user);
        assert!(doc["provider"].get("systemprompt").is_none(), "{doc}");
        assert!(doc["provider"].get("ollama").is_some(), "{doc}");
        assert!(doc.get("enabled_providers").is_none(), "{doc}");
        assert!(doc["mcp"].get("gateway-tools").is_none(), "{doc}");
        assert!(doc["mcp"].get("mine").is_some(), "{doc}");
        OPENCODE_HOST.remove_profile().expect("remove");
        assert!(!v1_plugin.exists(), "the V1 hooks plugin is removed");
    });
}

#[test]
fn a_managed_tier_provider_leaves_provider_and_policy_config_to_the_admin() {
    sandbox(|p| {
        std::fs::write(
            &p.managed,
            r#"{ "providers": { "systemprompt": { "package": "aisdk:@ai-sdk/openai-compatible" } } }"#,
        )
        .expect("seed mdm");
        std::fs::write(&p.user, r#"{ "theme": "dark" }"#).expect("seed user");
        install(&["claude-sonnet-5"]);
        assert_eq!(read(&p.user), serde_json::json!({ "theme": "dark" }));
        assert_eq!(
            std::fs::read_to_string(token_file(p)).expect("token"),
            "loopback-secret-value"
        );
        assert!(
            systemprompt_bridge::integration::opencode::managed_provider_source().is_some(),
            "doctor reports the MDM tier"
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
            .install_profile(
                &generated.path,
                &systemprompt_bridge::elevation_gate::ElevationGate::new(),
            )
            .expect_err("a foreign config aborts the install");
        assert!(err.to_string().contains("refusing"), "{err}");
        assert_eq!(
            std::fs::read_to_string(&p.user).expect("config"),
            "[1, 2, 3]"
        );
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
            doc["providers"]["systemprompt"]["models"]
                .get("claude-sonnet-5")
                .is_none(),
            "{doc}"
        );
        assert_eq!(doc["model"], "systemprompt/gpt-4.1");
    });
}

#[test]
fn foreign_policies_survive_after_ours() {
    sandbox(|p| {
        std::fs::write(
            &p.user,
            r#"{ "policies": [{ "effect": "deny", "action": "tool.use", "resource": "bash:*" }] }"#,
        )
        .expect("seed");
        install(&["claude-sonnet-5"]);
        install(&["claude-sonnet-5"]);
        let policies = read(&p.user)["policies"].clone();
        assert_eq!(policies.as_array().map(Vec::len), Some(3), "{policies}");
        assert_eq!(policies[2]["resource"], "bash:*");
        OPENCODE_HOST.remove_profile().expect("remove");
        assert_eq!(
            read(&p.user)["policies"],
            serde_json::json!([{ "effect": "deny", "action": "tool.use", "resource": "bash:*" }])
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
        assert_eq!(read(&p.user), serde_json::json!({ "theme": "dark" }));
        assert!(!token_file(p).exists());

        let again = OPENCODE_HOST.remove_profile().expect("second remove");
        assert!(
            matches!(again, ProfileRemoval::NothingToRemove),
            "{again:?}"
        );
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
