use std::collections::BTreeMap;

use systemprompt_bridge::integration::host_app::{HostApp, ProfileGenInputs};
use systemprompt_bridge::integration::opencode::OPENCODE_HOST;
use tempfile::TempDir;

fn sandbox<R>(f: impl FnOnce() -> R) -> R {
    let root = TempDir::new().expect("sandbox");
    let managed_dir = root.path().join("managed");
    std::fs::create_dir_all(&managed_dir).expect("managed dir");
    let vars: Vec<(&str, Option<String>)> = vec![
        ("HOME", Some(root.path().display().to_string())),
        (
            "XDG_CONFIG_HOME",
            Some(root.path().join("config").display().to_string()),
        ),
        (
            "XDG_DATA_HOME",
            Some(root.path().join("data").display().to_string()),
        ),
        (
            "SP_BRIDGE_OPENCODE_MANAGED_DIR",
            Some(managed_dir.display().to_string()),
        ),
    ];
    let out = temp_env::with_vars(vars, f);
    drop(root);
    out
}

fn rendered(models: &[&str], default_model: Option<&str>) -> serde_json::Value {
    let inputs = ProfileGenInputs {
        gateway_base_url: "http://127.0.0.1:48217".to_owned(),
        api_key: "loopback-secret-value".to_owned(),
        models: models.iter().map(|m| (*m).to_owned()).collect(),
        default_model: default_model.map(str::to_owned),
        organization_uuid: None,
        headers: BTreeMap::new(),
        mcp_servers: Vec::new(),
    };
    let generated = sandbox(|| {
        OPENCODE_HOST
            .generate_profile(&inputs)
            .expect("profile generated")
    });
    let body = std::fs::read_to_string(&generated.path).expect("generated readable");
    _ = std::fs::remove_file(&generated.path);
    serde_json::from_str(&body).expect("generated is JSON")
}

#[test]
fn default_model_from_profile_wins_over_first_listed() {
    let doc = rendered(
        &["claude-sonnet-5", "gemini-3.1-flash", "gpt-5"],
        Some("gpt-5"),
    );

    assert_eq!(
        doc["model"], "systemprompt/gpt-5",
        "with the whole catalog advertised the first entry is arbitrary; the \
         gateway's own default is the deliberate choice"
    );
}

#[test]
fn first_listed_when_default_is_not_advertised() {
    let doc = rendered(
        &["claude-sonnet-5", "gpt-5"],
        Some("a-model-nobody-advertises"),
    );

    assert_eq!(
        doc["model"], "systemprompt/claude-sonnet-5",
        "a default OpenCode cannot resolve would leave the picker broken"
    );

    let no_default = rendered(&["claude-sonnet-5", "gpt-5"], None);
    assert_eq!(no_default["model"], "systemprompt/claude-sonnet-5");
}
