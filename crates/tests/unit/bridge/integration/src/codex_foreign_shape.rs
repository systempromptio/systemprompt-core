//! A user's scalar at a key the bridge merges a table into is a foreign shape:
//! the install is refused with the key named and the file is left as found,
//! never rewritten around the conflict.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use systemprompt_bridge::ids::HostToken;
use systemprompt_bridge::integration::find_host_by_id;
use systemprompt_bridge::integration::host_app::ProfileGenInputs;

fn with_codex_home<R>(body: impl FnOnce(&Path) -> R) -> R {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir: PathBuf = temp.path().to_path_buf();
    let home_os: OsString = dir.clone().into();
    let system_cfg_os: OsString = dir.join("system_config.toml").into();
    temp_env::with_vars(
        [
            ("CODEX_HOME", Some(&home_os)),
            ("CODEX_SYSTEM_CONFIG", Some(&system_cfg_os)),
        ],
        || body(&dir),
    )
}

fn inputs() -> ProfileGenInputs {
    ProfileGenInputs {
        model_limits: Default::default(),
        gateway_base_url: "https://gateway.example.com".to_string(),
        host_token: HostToken::new("sp-test-key"),
        models: vec!["gpt-5".to_string()],
        default_model: None,
        organization_uuid: None,
        headers: Default::default(),
        mcp_servers: Some(Vec::new()),
    }
}

#[test]
fn a_scalar_where_the_bridge_owns_a_table_refuses_the_install_and_keeps_the_file() {
    if cfg!(target_os = "macos") {
        return;
    }
    with_codex_home(|home| {
        let target = if cfg!(target_os = "windows") {
            home.join("managed_config.toml")
        } else {
            home.join("system_config.toml")
        };
        // `model_providers` is where the bridge merges its provider table; a
        // user string there cannot be merged into.
        let seeded = "model_providers = \"not a table\"\nkeep = 1\n";
        fs::write(&target, seeded).unwrap();

        let host = find_host_by_id("codex-cli").expect("codex host registered");
        let profile = host.generate_profile(&inputs()).expect("generate");
        let err = host
            .install_profile(&profile.path)
            .expect_err("a foreign shape refuses the merge");

        let message = err.to_string();
        assert!(
            message.contains("model_providers") && message.contains("a table"),
            "the refusal names the key and the shape it needed: {message}"
        );
        assert_eq!(
            fs::read_to_string(&target).expect("target still readable"),
            seeded,
            "the user's file is left exactly as found"
        );
    });
}
