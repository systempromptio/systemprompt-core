use systemprompt_cli::admin::config::secret::set_secret;

fn write_secrets(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("secrets.json");
    std::fs::write(&path, content).expect("write secrets");
    (dir, path)
}

#[test]
fn set_secret_adds_key_to_file_missing_required_fields() {
    let (_dir, path) = write_secrets(r#"{ "anthropic": "old" }"#);

    set_secret(&path, "openai", "sk-new").expect("set permitted secret on incomplete file");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(written["openai"], "sk-new");
    assert_eq!(written["anthropic"], "old");
}

#[test]
fn set_secret_overwrites_existing_key() {
    let (_dir, path) = write_secrets(r#"{ "openai": "old" }"#);

    set_secret(&path, "openai", "sk-new").expect("overwrite");

    let written: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read")).expect("parse");
    assert_eq!(written["openai"], "sk-new");
}

#[test]
fn set_secret_rejects_reserved_infrastructure_secret() {
    let (_dir, path) = write_secrets(r#"{ "anthropic": "old" }"#);

    let err = set_secret(&path, "oauth_at_rest_pepper", "x")
        .expect_err("reserved infrastructure secret must be rejected");

    assert!(
        format!("{err:#}").contains("reserved infrastructure secret"),
        "got: {err:#}"
    );
}
