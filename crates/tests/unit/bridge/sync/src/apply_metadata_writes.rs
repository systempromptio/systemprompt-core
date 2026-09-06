//! The metadata fragments a sync writes beside the manifest, and the staging
//! directories it prepares first.

use std::path::Path;

use systemprompt_bridge::gateway::manifest::UserInfo;
use systemprompt_bridge::sync::apply::test_api::{prepare_dirs, write_mcp_servers, write_user};
use systemprompt_identifiers::UserId;

fn sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let home = tempfile::TempDir::new().expect("home");
    let state = tempfile::TempDir::new().expect("state");
    let root = home.path().to_path_buf();
    temp_env::with_vars(
        [
            ("HOME", Some(root.to_string_lossy().into_owned())),
            (
                "XDG_STATE_HOME",
                Some(state.path().to_string_lossy().into_owned()),
            ),
            (
                "XDG_DATA_HOME",
                Some(state.path().to_string_lossy().into_owned()),
            ),
            ("SUDO_USER", None),
        ],
        || f(&root),
    )
}

fn user() -> UserInfo {
    UserInfo {
        id: UserId::new("11111111-1111-4111-8111-111111111111"),
        name: "ed".to_owned(),
        email: "ed@example.invalid".to_owned(),
        display_name: Some("Ed".to_owned()),
        roles: vec!["admin".to_owned()],
    }
}

#[test]
fn preparing_directories_creates_the_root_the_metadata_dir_and_the_staging_dir() {
    sandbox(|home| {
        let root = home.join("org-plugins");
        let (meta, staging) = prepare_dirs(&root).expect("dirs are prepared");

        assert!(root.is_dir(), "the requested root is created");
        assert!(meta.is_dir(), "the metadata dir is created");
        assert!(staging.is_dir(), "the staging dir is created");
        assert_ne!(meta, staging);
    });
}

#[test]
fn preparing_directories_clears_anything_left_in_staging_by_an_earlier_run() {
    sandbox(|home| {
        let root = home.join("org-plugins");
        let (_, staging) = prepare_dirs(&root).expect("first prepare");
        let leftover = staging.join("half-downloaded-plugin");
        std::fs::create_dir_all(&leftover).expect("leftover from a crashed sync");

        let (_, staging_again) = prepare_dirs(&root).expect("second prepare");

        assert_eq!(staging, staging_again);
        assert!(
            !leftover.exists(),
            "staging must start empty so a crashed sync cannot leak into the next one"
        );
        assert!(staging_again.is_dir());
    });
}

#[test]
fn preparing_directories_is_idempotent_when_the_root_already_exists() {
    sandbox(|home| {
        let root = home.join("org-plugins");
        std::fs::create_dir_all(&root).expect("pre-existing root");
        let keep = root.join("existing-plugin");
        std::fs::create_dir_all(&keep).expect("existing content");

        prepare_dirs(&root).expect("prepare over an existing root");

        assert!(
            keep.is_dir(),
            "the plugin root is not cleared, only staging is"
        );
    });
}

#[test]
fn a_root_that_is_already_a_file_reports_which_path_could_not_be_created() {
    sandbox(|home| {
        let blocked = home.join("org-plugins");
        std::fs::write(&blocked, b"i am a file").expect("occupy the path");

        let err = prepare_dirs(&blocked).expect_err("a file cannot become the plugin root");
        let rendered = err.to_string();
        assert!(
            rendered.contains("org-plugins"),
            "the error must name the path, got {rendered}"
        );
    });
}

#[test]
fn a_user_fragment_is_written_as_pretty_json_that_reads_back_as_the_same_user() {
    sandbox(|home| {
        let (meta, _) = prepare_dirs(&home.join("org-plugins")).expect("prepare");
        write_user(&meta, Some(&user())).expect("write the user fragment");

        let text = std::fs::read_to_string(meta.join("user.json")).expect("read back");
        assert!(text.contains('\n'), "the fragment is pretty-printed");

        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["email"], "ed@example.invalid");
        assert_eq!(parsed["display_name"], "Ed");
        assert_eq!(parsed["roles"][0], "admin");
    });
}

#[test]
fn a_signed_out_sync_writes_a_null_user_fragment_rather_than_leaving_the_old_one() {
    sandbox(|home| {
        let (meta, _) = prepare_dirs(&home.join("org-plugins")).expect("prepare");
        write_user(&meta, Some(&user())).expect("write a user");
        write_user(&meta, None).expect("write no user");

        let text = std::fs::read_to_string(meta.join("user.json")).expect("read back");
        assert_eq!(
            text.trim(),
            "null",
            "a stale identity must not survive a signed-out sync"
        );
    });
}

#[test]
fn writing_the_user_fragment_into_a_directory_that_is_not_there_reports_the_path() {
    sandbox(|home| {
        let absent = home.join("no-such-metadata-dir");
        let err = write_user(&absent, Some(&user())).expect_err("the directory does not exist");
        assert!(
            err.to_string().contains("user.json"),
            "the error must name the fragment it failed to write, got {err}"
        );
    });
}

#[test]
fn an_empty_managed_server_list_writes_an_empty_json_array() {
    sandbox(|home| {
        let (meta, _) = prepare_dirs(&home.join("org-plugins")).expect("prepare");
        write_mcp_servers(&meta, &[]).expect("write no servers");

        let text = std::fs::read_to_string(meta.join("mcp-servers.json")).expect("read back");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(
            parsed.as_array().map(Vec::len),
            Some(0),
            "an empty list is an empty array, not null or a missing file"
        );
    });
}

#[test]
fn writing_the_server_fragment_into_a_directory_that_is_not_there_reports_the_path() {
    sandbox(|home| {
        let absent = home.join("no-such-metadata-dir");
        let err = write_mcp_servers(&absent, &[]).expect_err("the directory does not exist");
        assert!(
            err.to_string().contains("mcp-servers.json"),
            "the error must name the fragment it failed to write, got {err}"
        );
    });
}

#[test]
fn rewriting_a_fragment_replaces_it_rather_than_appending() {
    sandbox(|home| {
        let (meta, _) = prepare_dirs(&home.join("org-plugins")).expect("prepare");
        write_mcp_servers(&meta, &[]).expect("first write");
        write_mcp_servers(&meta, &[]).expect("second write");

        let text = std::fs::read_to_string(meta.join("mcp-servers.json")).expect("read back");
        serde_json::from_str::<serde_json::Value>(&text)
            .expect("a rewritten fragment is still one valid document");
    });
}
