//! Seeding the organisation's default model into Claude Code's settings file
//! without overwriting a choice the user made themselves.

#![cfg(not(any(target_os = "macos", target_os = "windows")))]

use std::path::{Path, PathBuf};

use systemprompt_bridge::install::mdm::linux::settings::test_api::{
    managed_settings_path, seed_default_model,
};

fn sandbox<R>(f: impl FnOnce(&Path) -> R) -> R {
    let home = tempfile::TempDir::new().expect("home");
    let root = home.path().to_path_buf();
    temp_env::with_vars(
        [
            ("HOME", Some(root.to_string_lossy().into_owned())),
            ("SUDO_USER", None),
        ],
        || f(&root),
    )
}

fn user_settings(home: &Path) -> PathBuf {
    home.join(".claude").join("settings.json")
}

fn write_settings(home: &Path, body: &str) {
    let dir = home.join(".claude");
    std::fs::create_dir_all(&dir).expect(".claude");
    std::fs::write(dir.join("settings.json"), body).expect("settings");
}

#[test]
fn without_write_access_to_the_system_path_the_settings_land_under_the_users_home() {
    sandbox(|home| {
        let path = managed_settings_path().expect("a path is always resolvable with a home");
        assert!(
            path == PathBuf::from("/etc/claude-code/managed-settings.json")
                || path == user_settings(home),
            "expected either the system path or the per-user fallback, got {}",
            path.display()
        );
    });
}

#[test]
fn seeding_into_a_fresh_install_writes_the_model_and_reports_that_it_did() {
    sandbox(|home| {
        let seeded = seed_default_model("claude-opus-5").expect("seed");
        assert!(seeded, "a fresh install has no model to preserve");

        let path = managed_settings_path().expect("path");
        if path != user_settings(home) {
            return;
        }
        let text = std::fs::read_to_string(&path).expect("read back");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["model"], "claude-opus-5");
        assert!(text.ends_with('\n'), "the file ends with a newline");
    });
}

#[test]
fn a_model_the_user_already_chose_is_left_alone_and_the_seed_reports_no_change() {
    sandbox(|home| {
        write_settings(home, r#"{"model": "claude-haiku-4-5"}"#);
        if managed_settings_path().as_deref() != Some(&user_settings(home)) {
            return;
        }

        let seeded = seed_default_model("claude-opus-5").expect("seed");
        assert!(!seeded, "an existing choice must not be overwritten");

        let text = std::fs::read_to_string(user_settings(home)).expect("read back");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(
            parsed["model"], "claude-haiku-4-5",
            "the user's own /model choice survives every sync"
        );
    });
}

#[test]
fn seeding_preserves_every_other_setting_already_in_the_file() {
    sandbox(|home| {
        write_settings(
            home,
            r#"{"theme": "dark", "verbose": true, "nested": {"a": 1}}"#,
        );
        if managed_settings_path().as_deref() != Some(&user_settings(home)) {
            return;
        }

        assert!(seed_default_model("claude-opus-5").expect("seed"));

        let text = std::fs::read_to_string(user_settings(home)).expect("read back");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["model"], "claude-opus-5");
        assert_eq!(parsed["theme"], "dark");
        assert_eq!(parsed["verbose"], true);
        assert_eq!(parsed["nested"]["a"], 1);
    });
}

#[test]
fn an_empty_settings_file_is_treated_as_an_empty_object_rather_than_a_parse_error() {
    sandbox(|home| {
        write_settings(home, "   \n  ");
        if managed_settings_path().as_deref() != Some(&user_settings(home)) {
            return;
        }

        assert!(seed_default_model("claude-opus-5").expect("whitespace is not a parse error"));
        let text = std::fs::read_to_string(user_settings(home)).expect("read back");
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["model"], "claude-opus-5");
    });
}

#[test]
fn a_settings_file_that_is_not_valid_json_is_reported_and_names_the_path() {
    sandbox(|home| {
        write_settings(home, "{ this is not json");
        if managed_settings_path().as_deref() != Some(&user_settings(home)) {
            return;
        }

        let err = seed_default_model("claude-opus-5")
            .expect_err("a malformed settings file must not be silently overwritten");
        let rendered = err.to_string();
        assert!(
            rendered.contains("settings.json"),
            "the error must name the file, got {rendered}"
        );
        assert!(rendered.contains("not valid JSON"), "got {rendered}");
    });
}

#[test]
fn a_settings_file_holding_a_json_array_is_refused_rather_than_replaced() {
    sandbox(|home| {
        write_settings(home, r#"["not", "an", "object"]"#);
        if managed_settings_path().as_deref() != Some(&user_settings(home)) {
            return;
        }

        let err = seed_default_model("claude-opus-5")
            .expect_err("settings must be a JSON object, not an array");
        assert!(err.to_string().contains("settings.json"), "got {err}");
    });
}

#[test]
fn seeding_twice_is_idempotent_because_the_second_run_sees_its_own_write() {
    sandbox(|home| {
        if managed_settings_path().as_deref() != Some(&user_settings(home)) {
            return;
        }
        assert!(seed_default_model("claude-opus-5").expect("first seed"));
        assert!(
            !seed_default_model("claude-opus-5").expect("second seed"),
            "the second run finds the key it wrote and leaves it alone"
        );
    });
}
