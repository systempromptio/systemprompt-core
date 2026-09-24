//! An admin-tier `opencode.json` left behind by an elevated install is
//! read-only to the tray. An unattended sync must neither fail nor leave the
//! live catalogue unwritten: it writes the provider block to the user tier,
//! says so in a warning, and leaves the admin file untouched. Attended runs
//! that are refused fall back the same way.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs, ProfileInstalled};
use systemprompt_bridge::integration::opencode::{OPENCODE_HOST, admin_tier_models};
use tempfile::TempDir;

const STALE_ADMIN: &str = r#"{
  "provider": { "systemprompt": {
    "npm": "@ai-sdk/openai-compatible",
    "options": { "baseURL": "http://127.0.0.1:48217/v1" },
    "models": { "claude-sonnet-4-5": { "name": "claude-sonnet-4-5" } }
  } },
  "model": "systemprompt/claude-sonnet-4-5"
}"#;

struct Paths {
    managed: PathBuf,
    user: PathBuf,
}

fn sandbox<R>(f: impl FnOnce(&Paths) -> R) -> Option<R> {
    let root = TempDir::new().expect("sandbox");
    let managed_dir = root.path().join("managed");
    std::fs::create_dir_all(&managed_dir).expect("managed dir");
    let config_home = root.path().join("config");
    let bridge_dir = config_home.join("systemprompt");
    std::fs::create_dir_all(&bridge_dir).expect("bridge config dir");
    std::fs::write(
        bridge_dir.join("systemprompt-bridge.toml"),
        format!("[opencode]\nmanaged_dir = '{}'\n", managed_dir.display()),
    )
    .expect("bridge config");
    let paths = Paths {
        managed: managed_dir.join("opencode.json"),
        user: config_home.join("opencode").join("opencode.json"),
    };
    std::fs::write(&paths.managed, STALE_ADMIN).expect("seed admin tier");
    std::fs::set_permissions(&paths.managed, std::fs::Permissions::from_mode(0o444))
        .expect("read-only admin tier");
    // The atomic write renames a sibling temp file, so the directory must
    // refuse it too, as /etc and %ProgramData% do for a normal user.
    std::fs::set_permissions(&managed_dir, std::fs::Permissions::from_mode(0o555))
        .expect("read-only admin dir");
    // Root ignores the mode bits, so the scenario cannot be staged.
    if std::fs::OpenOptions::new().write(true).open(&paths.managed).is_ok() {
        std::fs::set_permissions(&managed_dir, std::fs::Permissions::from_mode(0o755))
            .expect("restore admin dir for cleanup");
        return None;
    }
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        ("XDG_CONFIG_HOME", Some(config_home.display().to_string())),
        ("XDG_DATA_HOME", Some(root.path().join("data").display().to_string())),
        ("SP_BRIDGE_CONFIG", None),
    ];
    let out = temp_env::with_vars(vars, || f(&paths));
    std::fs::set_permissions(&managed_dir, std::fs::Permissions::from_mode(0o755))
        .expect("restore admin dir for cleanup");
    drop(root);
    Some(out)
}

fn generated() -> String {
    OPENCODE_HOST
        .generate_profile(&ProfileGenInputs {
            model_limits: Default::default(),
            gateway_base_url: "http://127.0.0.1:48217".to_owned(),
            host_token: HostToken::new("loopback-secret-value"),
            models: vec!["claude-opus-5-5".to_owned(), "claude-sonnet-5".to_owned()],
            default_model: None,
            organization_uuid: None,
            headers: BTreeMap::new(),
            mcp_servers: Some(Vec::new()),
        })
        .expect("generate")
        .path
}

fn read(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("readable")).expect("json")
}

fn assert_user_tier_carries_live_catalogue(p: &Paths, installed: &ProfileInstalled) {
    let user = read(&p.user);
    let models = user["provider"]["systemprompt"]["models"]
        .as_object()
        .expect("models object");
    assert!(models.contains_key("claude-opus-5-5"), "{user}");
    assert!(models.contains_key("claude-sonnet-5"), "{user}");
    assert_eq!(
        std::fs::read_to_string(&p.managed).expect("admin readable"),
        STALE_ADMIN,
        "the admin tier is left as it was"
    );
    assert_eq!(installed.warnings.len(), 1, "{:?}", installed.warnings);
    assert!(
        installed.warnings[0].contains("read-only"),
        "{:?}",
        installed.warnings
    );
}

#[test]
fn an_unattended_sync_writes_the_live_catalogue_to_the_user_tier() {
    let _ = sandbox(|p| {
        let installed = OPENCODE_HOST
            .install_profile_unattended(&generated())
            .expect("unattended install succeeds");
        assert_user_tier_carries_live_catalogue(p, &installed);
    });
}

#[test]
fn a_refused_attended_install_falls_back_to_the_user_tier() {
    let _ = sandbox(|p| {
        let installed = OPENCODE_HOST
            .install_profile(&generated())
            .expect("attended install falls back");
        assert_user_tier_carries_live_catalogue(p, &installed);
    });
}

#[test]
fn the_admin_tier_model_list_is_readable_for_the_doctor() {
    let _ = sandbox(|p| {
        let (path, models) = admin_tier_models().expect("admin tier has a provider block");
        assert_eq!(path, p.managed);
        assert_eq!(models, vec!["claude-sonnet-4-5".to_owned()]);
    });
}
