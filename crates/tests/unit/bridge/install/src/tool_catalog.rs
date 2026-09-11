//! The per-server tool catalog behind Claude Desktop's `toolPolicy`: absence
//! is empty, corruption is an error, and a write never rebuilds from a
//! catalog it could not read.

use std::path::PathBuf;

use systemprompt_bridge::install::mdm::tool_catalog;
use tempfile::TempDir;

fn in_sandbox<R>(state: &TempDir, f: impl FnOnce() -> R) -> R {
    let root = state.path().display().to_string();
    temp_env::with_vars(
        vec![
            ("HOME", Some(root.clone())),
            ("XDG_STATE_HOME", Some(root.clone())),
            ("XDG_CONFIG_HOME", Some(root)),
        ],
        f,
    )
}

fn catalog_file(state: &TempDir) -> PathBuf {
    let meta = state.path().join("systemprompt-bridge").join("metadata");
    std::fs::create_dir_all(&meta).expect("metadata dir");
    meta.join("mcp-tools.json")
}

#[test]
fn an_absent_catalog_reads_as_empty() {
    let state = TempDir::new().expect("state");
    let catalog = in_sandbox(&state, || tool_catalog::read().expect("absent is empty"));
    assert!(catalog.is_empty());
}

#[test]
fn a_corrupt_catalog_is_an_error_and_is_not_overwritten_by_a_retain() {
    let state = TempDir::new().expect("state");
    let path = catalog_file(&state);
    std::fs::write(&path, "{ not json").expect("seed");
    in_sandbox(&state, || {
        let err = tool_catalog::read().expect_err("corruption is not an empty catalog");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("mcp-tools.json"), "{err}");
        let err = tool_catalog::retain(&["atlassian".to_owned()])
            .expect_err("a retain over an unreadable catalog must not rebuild it");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    });
    assert_eq!(
        std::fs::read_to_string(&path).expect("still there"),
        "{ not json",
        "the corrupt file is left for the operator, not replaced with an empty catalog"
    );
}

#[test]
fn retain_drops_servers_that_left_the_manifest_and_keeps_the_rest() {
    let state = TempDir::new().expect("state");
    let path = catalog_file(&state);
    std::fs::write(
        &path,
        r#"{"atlassian":["read_issue"],"retired":["old_tool"]}"#,
    )
    .expect("seed");
    in_sandbox(&state, || {
        tool_catalog::retain(&["atlassian".to_owned()]).expect("retain");
        let catalog = tool_catalog::read().expect("read back");
        assert_eq!(
            catalog.keys().cloned().collect::<Vec<_>>(),
            vec!["atlassian"]
        );
        assert_eq!(catalog["atlassian"], vec!["read_issue"]);
    });
}
