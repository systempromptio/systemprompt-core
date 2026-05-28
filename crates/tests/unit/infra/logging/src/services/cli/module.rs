//! Unit tests for cli::module display helpers.

use systemprompt_logging::services::cli::display::Display;
use systemprompt_logging::services::cli::module::{
    BatchModuleOperations, ModuleDisplay, ModuleInstall, ModuleUpdate,
};

#[test]
fn module_install_new_minimal() {
    let m = ModuleInstall::new("mod", "1.0.0");
    assert_eq!(m.name, "mod");
    assert_eq!(m.version, "1.0.0");
    assert!(m.description.is_none());
}

#[test]
fn module_install_with_description_sets_field() {
    let m = ModuleInstall::new("mod", "1.0.0").with_description("desc");
    assert_eq!(m.description.as_deref(), Some("desc"));
}

#[test]
fn module_install_display_smoke() {
    ModuleInstall::new("alpha", "0.1.0").display();
    ModuleInstall::new("beta", "0.2.0")
        .with_description("a thing")
        .display();
}

#[test]
fn module_update_new() {
    let u = ModuleUpdate::new("mod", "1.0.0", "1.1.0");
    assert_eq!(u.name, "mod");
    assert_eq!(u.old_version, "1.0.0");
    assert_eq!(u.new_version, "1.1.0");
    assert!(u.changes.is_empty());
}

#[test]
fn module_update_with_change_accumulates() {
    let u = ModuleUpdate::new("m", "1", "2")
        .with_change("c1")
        .with_change("c2");
    assert_eq!(u.changes, vec!["c1".to_owned(), "c2".to_owned()]);
}

#[test]
fn module_update_with_changes_replaces() {
    let u = ModuleUpdate::new("m", "1", "2")
        .with_change("ignored")
        .with_changes(vec!["a".to_owned(), "b".to_owned()]);
    assert_eq!(u.changes, vec!["a".to_owned(), "b".to_owned()]);
}

#[test]
fn module_update_display_smoke() {
    ModuleUpdate::new("m", "1", "2")
        .with_change("changelog 1")
        .with_change("changelog 2")
        .display();
}

#[test]
fn module_display_missing_schemas_empty_short_circuits() {
    ModuleDisplay::missing_schemas("mymod", &[]);
}

#[test]
fn module_display_missing_schemas_renders() {
    ModuleDisplay::missing_schemas(
        "mymod",
        &[
            ("a.sql".to_owned(), "table_a".to_owned()),
            ("b.sql".to_owned(), "table_b".to_owned()),
        ],
    );
}

#[test]
fn module_display_missing_seeds_empty_short_circuits() {
    ModuleDisplay::missing_seeds("m", &[]);
}

#[test]
fn module_display_missing_seeds_renders() {
    ModuleDisplay::missing_seeds("m", &[("seed.sql".to_owned(), "users".to_owned())]);
}

#[test]
fn prompt_apply_schemas_empty_returns_ok_false() {
    let r = ModuleDisplay::prompt_apply_schemas("m", &[]).unwrap();
    assert!(!r);
}

#[test]
fn prompt_apply_seeds_empty_returns_ok_false() {
    let r = ModuleDisplay::prompt_apply_seeds("m", &[]).unwrap();
    assert!(!r);
}

#[test]
fn batch_install_empty_returns_ok_false() {
    let r = BatchModuleOperations::prompt_install_multiple(&[]).unwrap();
    assert!(!r);
}

#[test]
fn batch_update_empty_returns_ok_false() {
    let r = BatchModuleOperations::prompt_update_multiple(&[]).unwrap();
    assert!(!r);
}
