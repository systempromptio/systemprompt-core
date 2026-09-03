use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileRemoval};
use systemprompt_bridge::integration::opencode::OPENCODE_HOST;
use tempfile::TempDir;

struct Paths {
    managed: PathBuf,
    auth: PathBuf,
}

fn sandbox<R>(f: impl FnOnce(&Paths) -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed_dir = root.path().join("managed");
    std::fs::create_dir_all(&managed_dir).expect("managed dir");
    let data = root.path().join("data");
    let paths = Paths {
        managed: managed_dir.join("opencode.json"),
        auth: data.join("opencode").join("auth.json"),
    };
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        (
            "XDG_CONFIG_HOME",
            Some(root.path().join("config").display().to_string()),
        ),
        ("XDG_DATA_HOME", Some(data.display().to_string())),
        (
            "SP_BRIDGE_OPENCODE_MANAGED_DIR",
            Some(managed_dir.display().to_string()),
        ),
    ];
    let out = temp_env::with_vars(vars, || f(&paths));
    drop(root);
    out
}

fn inputs(models: &[&str]) -> ProfileGenInputs {
    let mut headers = BTreeMap::new();
    headers.insert("x-inference-protocol".to_owned(), "openai".to_owned());
    ProfileGenInputs {
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        api_key: "loopback-secret-value".to_owned(),
        models: models.iter().map(|m| (*m).to_owned()).collect(),
        organization_uuid: Some("org-abc".to_owned()),
        headers,
        mcp_servers: Vec::new(),
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
fn installing_merges_the_provider_block_and_preserves_foreign_keys() {
    sandbox(|p| {
        std::fs::write(
            &p.managed,
            r#"{ "theme": "dark", "provider": { "ollama": { "npm": "@ai-sdk/openai-compatible" } }, "mcp": { "mine": { "type": "remote", "url": "https://example.com/mcp" } } }"#,
        )
        .expect("seed");
        install(&["claude-sonnet-5", "gpt-4.1"]);
        let doc = read(&p.managed);
        assert_eq!(doc["theme"], "dark", "{doc}");
        assert_eq!(
            doc["provider"]["ollama"]["npm"],
            "@ai-sdk/openai-compatible"
        );
        assert_eq!(doc["mcp"]["mine"]["url"], "https://example.com/mcp");
        assert_eq!(
            doc["provider"]["systemprompt"]["options"]["baseURL"],
            "http://127.0.0.1:48217/v1"
        );
        assert_eq!(
            doc["provider"]["systemprompt"]["models"]["gpt-4.1"]["name"],
            "gpt-4.1"
        );
        assert_eq!(doc["model"], "systemprompt/claude-sonnet-5");
        assert!(
            doc.get("_systemprompt_api_key").is_none(),
            "the key marker never reaches the managed file: {doc}"
        );
    });
}

#[test]
fn the_api_key_lands_in_auth_json_with_other_providers_kept() {
    sandbox(|p| {
        std::fs::create_dir_all(p.auth.parent().expect("parent")).expect("data dir");
        std::fs::write(
            &p.auth,
            r#"{ "anthropic": { "type": "oauth", "refresh": "r", "access": "a", "expires": 1 } }"#,
        )
        .expect("seed auth");
        install(&["claude-sonnet-5"]);
        let auth = read(&p.auth);
        assert_eq!(auth["systemprompt"]["type"], "api");
        assert_eq!(auth["systemprompt"]["key"], "loopback-secret-value");
        assert_eq!(auth["anthropic"]["type"], "oauth", "{auth}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p.auth)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "auth.json holds a secret");
        }
    });
}

#[test]
fn a_non_object_auth_json_is_never_overwritten() {
    sandbox(|p| {
        std::fs::create_dir_all(p.auth.parent().expect("parent")).expect("data dir");
        std::fs::write(&p.auth, "[1, 2, 3]").expect("seed auth");
        let generated = OPENCODE_HOST
            .generate_profile(&inputs(&["claude-sonnet-5"]))
            .expect("generate");
        let err = OPENCODE_HOST
            .install_profile(&generated.path)
            .expect_err("a foreign auth.json aborts the install");
        assert!(err.to_string().contains("refusing"), "{err}");
        assert_eq!(std::fs::read_to_string(&p.auth).expect("auth"), "[1, 2, 3]");
        _ = std::fs::remove_file(&generated.path);
    });
}

#[test]
fn a_shrunk_model_list_leaves_no_stale_entries() {
    sandbox(|p| {
        install(&["claude-sonnet-5", "gpt-4.1"]);
        install(&["gpt-4.1"]);
        let doc = read(&p.managed);
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
        let doc = read(&p.managed);
        assert!(doc.get("model").is_none(), "{doc}");

        std::fs::write(&p.managed, r#"{ "model": "anthropic/claude-opus-5" }"#)
            .expect("seed foreign default");
        install(&[]);
        let doc = read(&p.managed);
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
        let first = std::fs::read(&p.managed).expect("first");
        let first_auth = std::fs::read(&p.auth).expect("first auth");
        install(&["claude-sonnet-5"]);
        assert_eq!(std::fs::read(&p.managed).expect("second"), first);
        assert_eq!(std::fs::read(&p.auth).expect("second auth"), first_auth);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p.managed)
                .expect("meta")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                mode, 0o644,
                "the managed file must stay readable by the host"
            );
        }
    });
}

#[test]
fn removing_strips_only_our_keys_and_reports_what_happened() {
    sandbox(|p| {
        std::fs::write(&p.managed, r#"{ "theme": "dark" }"#).expect("seed");
        std::fs::create_dir_all(p.auth.parent().expect("parent")).expect("data dir");
        std::fs::write(&p.auth, r#"{ "openai": { "type": "api", "key": "sk" } }"#)
            .expect("seed auth");
        install(&["claude-sonnet-5"]);

        let removal = OPENCODE_HOST.remove_profile().expect("remove");
        assert!(
            matches!(removal, ProfileRemoval::Removed { .. }),
            "{removal:?}"
        );
        let doc = read(&p.managed);
        assert_eq!(doc, serde_json::json!({ "theme": "dark" }), "{doc}");
        let auth = read(&p.auth);
        assert_eq!(
            auth,
            serde_json::json!({ "openai": { "type": "api", "key": "sk" } })
        );

        let again = OPENCODE_HOST.remove_profile().expect("second remove");
        assert!(
            matches!(again, ProfileRemoval::NothingToRemove),
            "{again:?}"
        );
    });
}

#[test]
fn removing_the_only_content_deletes_both_files() {
    sandbox(|p| {
        install(&["claude-sonnet-5"]);
        OPENCODE_HOST.remove_profile().expect("remove");
        assert!(!p.managed.exists(), "an empty managed file must not linger");
        assert!(!p.auth.exists(), "an empty auth.json must not linger");
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
