use std::fs;

use harness_config::{ConfigError, Layout, Settings, update_global_setting};
use serde_json::json;

use crate::support::{load, paths, repo, write};

#[test]
fn layers_merge_objects_and_replace_scalars_and_arrays() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("harness.json"),
        r#"{"theme": {"a": 1, "b": 2}, "d": "x", "experimental": {"list": [1, 2]},
            "compaction": {"auto": false, "reserved_tokens": 5}}"#,
    );
    write(
        &repo.join(".harness").join("harness.json"),
        r#"{"theme": {"b": 3}, "d": null, "experimental": {"list": [3]}, "compaction": {"prune": false}}"#,
    );
    let settings = load(&paths, &repo).settings;
    assert_eq!(settings.extra["theme"], json!({"a": 1, "b": 3}));
    assert_eq!(settings.extra["d"], json!(null));
    assert_eq!(settings.experimental["list"], json!([3]));
    assert!(!settings.compaction.auto);
    assert!(!settings.compaction.prune);
    assert_eq!(settings.compaction.reserved_tokens, Some(5));
}

#[test]
fn edit_keeps_comments_and_creates_parents() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let layout = Layout::default();
    let file = layout.global_settings(&paths);
    write(&file, "{\n  // chosen model\n  \"model\": \"a\",\n}\n");
    assert!(update_global_setting(&paths, &layout, &["model"], Some(&json!("b"))).unwrap());
    let text = fs::read_to_string(&file).unwrap();
    assert!(text.contains("// chosen model"), "{text}");
    assert_eq!(load(&paths, &repo).settings.model.as_deref(), Some("b"));

    assert!(
        update_global_setting(
            &paths,
            &layout,
            &["compaction", "auto"],
            Some(&json!(false))
        )
        .unwrap()
    );
    assert!(update_global_setting(&paths, &layout, &["model"], None).unwrap());
    let settings = load(&paths, &repo).settings;
    assert_eq!(settings.model, None);
    assert!(!settings.compaction.auto);
    assert!(
        fs::read_to_string(&file)
            .unwrap()
            .contains("// chosen model")
    );
    assert!(matches!(
        update_global_setting(&paths, &layout, &[], None),
        Err(ConfigError::EmptyKey)
    ));
}

#[test]
fn edit_of_a_missing_file_starts_an_object() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let layout = Layout::default();
    assert!(update_global_setting(&paths, &layout, &["shell"], Some(&json!("pwsh"))).unwrap());
    assert_eq!(load(&paths, &repo).settings.shell.as_deref(), Some("pwsh"));

    let other = tempfile::tempdir().unwrap();
    let fresh = crate::support::paths(other.path());
    update_global_setting(&fresh, &layout, &["missing", "key"], None).unwrap();
    assert_eq!(load(&fresh, &repo).settings, Settings::default());
}

#[test]
fn global_update_validates_and_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    let layout = Layout::default();
    assert!(update_global_setting(&paths, &layout, &["model"], Some(&json!("m1"))).unwrap());
    assert!(!update_global_setting(&paths, &layout, &["model"], Some(&json!("m1"))).unwrap());
    let refused = update_global_setting(&paths, &layout, &["model"], Some(&json!(5)));
    assert!(refused.is_err());
    let loaded = load(&paths, &repo);
    assert_eq!(loaded.settings.model.as_deref(), Some("m1"));
    assert_eq!(loaded.settings_files, [layout.global_settings(&paths)]);
}

#[test]
fn parse_errors_carry_a_line() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = paths(tmp.path());
    let repo = repo(tmp.path());
    write(
        &paths.config.join("harness.json"),
        "{\n  \"a\": 1,\n  \"b\": ]\n}",
    );
    let error =
        harness_config::Loaded::load(&paths, &Layout::default(), &repo, Vec::new()).unwrap_err();
    assert_eq!(error.line(), Some(3));
}
