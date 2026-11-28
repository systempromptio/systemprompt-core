//! Noticing configuration changes: settings files that appear, change, are
//! recreated or are reached through symlinks, and changes that do not count.

use crate::support::{Tree, touch_later};

#[test]
fn a_settings_file_that_changes_makes_the_load_stale() {
    let tree = Tree::new();
    let file = tree.project("harness.json", r#"{"model": "a"}"#);
    let loaded = tree.load();
    std::fs::write(&file, r#"{"model": "b"}"#).unwrap();
    touch_later(&file);
    assert!(loaded.is_stale());
    assert_eq!(tree.load().settings.model.as_deref(), Some("b"));
}

#[test]
fn a_deleted_then_recreated_settings_file_is_noticed_both_times() {
    let tree = Tree::new();
    let file = tree.project("harness.json", r#"{"model": "a"}"#);
    let loaded = tree.load();
    std::fs::remove_file(&file).unwrap();
    assert!(loaded.is_stale());
    let gone = tree.load();
    assert_eq!(gone.settings.model, None);
    tree.project("harness.json", r#"{"model": "again"}"#);
    assert!(gone.is_stale());
    assert_eq!(tree.load().settings.model.as_deref(), Some("again"));
}

#[test]
fn the_first_settings_file_written_after_start_up_is_loaded() {
    let tree = Tree::new();
    let loaded = tree.load();
    tree.global("harness.json", r#"{"model": "late"}"#);
    assert!(loaded.is_stale());
    assert_eq!(tree.load().settings.model.as_deref(), Some("late"));
}

#[test]
fn a_settings_directory_created_after_start_up_is_still_watched() {
    let tree = Tree::new();
    assert!(!tree.paths.config.exists());
    let loaded = tree.load();
    tree.global("harness.json", r#"{"model": "made"}"#);
    assert!(loaded.is_stale());
}

#[test]
fn an_unrelated_file_change_does_not_make_the_load_stale() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"model": "a"}"#);
    let loaded = tree.load();
    tree.project("src/main.rs", "fn main() {}");
    tree.global("unrelated.txt", "x");
    assert!(!loaded.is_stale());
}

#[test]
fn a_reload_that_changes_nothing_reports_nothing_changed() {
    let tree = Tree::new();
    tree.project("harness.json", r#"{"model": "a"}"#);
    let first = tree.load();
    let second = tree.load();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.settings, second.settings);
}

#[test]
fn candidate_files_that_do_not_exist_yet_are_part_of_what_a_load_watches() {
    let tree = Tree::new();
    let loaded = tree.load();
    let watched = loaded.inputs();
    assert!(watched.contains(&tree.repo.join("harness.json")));
    assert!(watched.contains(&tree.repo.join(".harness").join("harness.json")));
    assert!(watched.contains(&tree.paths.config.join("harness.json")));
}

#[cfg(unix)]
#[test]
fn a_settings_file_reached_through_a_symlink_reloads_when_its_target_changes() {
    let tree = Tree::new();
    let target = tree.put(&tree.paths.data.join("real.json"), r#"{"model": "one"}"#);
    std::fs::create_dir_all(&tree.paths.config).unwrap();
    std::os::unix::fs::symlink(&target, tree.paths.config.join("harness.json")).unwrap();
    let loaded = tree.load();
    assert_eq!(loaded.settings.model.as_deref(), Some("one"));
    std::fs::write(&target, r#"{"model": "two"}"#).unwrap();
    touch_later(&target);
    assert!(loaded.is_stale());
    assert_eq!(tree.load().settings.model.as_deref(), Some("two"));
}

#[test]
fn a_directory_removed_and_created_again_at_the_same_path_is_seen_afresh() {
    let tree = Tree::new();
    let dir = tree.repo.join(".harness/commands");
    tree.project(".harness/commands/a.md", "a");
    let loaded = tree.load();
    std::fs::remove_dir_all(&dir).unwrap();
    tree.project(".harness/commands/b.md", "b");
    assert!(loaded.is_stale());
    assert_eq!(tree.load().commands.keys().collect::<Vec<_>>(), ["b"]);
}

#[cfg(unix)]
#[test]
fn a_permission_change_on_a_settings_file_is_noticed_as_a_change_only_if_it_changes_content() {
    use std::os::unix::fs::PermissionsExt;
    let tree = Tree::new();
    let file = tree.project("harness.json", r#"{"model": "a"}"#);
    let loaded = tree.load();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let reloaded = tree.load();
    assert_eq!(reloaded.settings, loaded.settings);
}

#[cfg(unix)]
#[test]
fn settings_in_an_ancestor_that_cannot_be_listed_are_still_found() {
    use std::os::unix::fs::PermissionsExt as _;
    let tree = Tree::new();
    let guarded = tree.repo.join("guarded");
    let project = guarded.join("project");
    std::fs::create_dir_all(&project).unwrap();
    tree.put(
        &guarded.join(".harness").join("harness.json"),
        r#"{"model": "guarded"}"#,
    );
    std::fs::set_permissions(&guarded, std::fs::Permissions::from_mode(0o311)).unwrap();
    let loaded = tree.load_at(&project);
    std::fs::set_permissions(&guarded, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(loaded.settings.model.as_deref(), Some("guarded"));
}
