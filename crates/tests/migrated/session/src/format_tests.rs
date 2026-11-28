//! Formatter state across a reload: a cleared formatter resolves its commands
//! afresh, and a command found before the reload is not kept after it.

use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use systemprompt_engine::workspace::{Formatter, FormatterConfig, FormatterSpec};

fn formatter(worktree: &Path, bin: &Path) -> Formatter {
    let spec = FormatterSpec {
        name: "tidy".to_owned(),
        command: "tidy-fmt".to_owned(),
        args: vec!["$FILE".to_owned()],
        extensions: vec!["txt".to_owned()],
        root_markers: Vec::new(),
        node_modules: false,
    };
    Formatter::new(
        worktree,
        FormatterConfig {
            builtins: false,
            custom: vec![spec],
            search_path: Some(OsString::from(bin)),
            ..FormatterConfig::default()
        },
    )
}

fn install(bin: &Path) {
    let program = bin.join("tidy-fmt");
    fs::write(&program, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_cleared_formatter_resolves_a_command_installed_since_the_last_lookup() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let file = dir.path().join("notes.txt");
    let formatter = formatter(dir.path(), &bin);
    assert_eq!(formatter.resolve(&file), None);
    install(&bin);
    assert_eq!(formatter.resolve(&file), None);
    formatter.clear_cache();
    let resolved = formatter
        .resolve(&file)
        .expect("the installed command resolves");
    assert_eq!(resolved.program, bin.join("tidy-fmt"));
}

#[test]
fn a_command_resolved_before_a_clear_is_not_kept_after_it() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    install(&bin);
    let file = dir.path().join("notes.txt");
    let formatter = formatter(dir.path(), &bin);
    assert!(formatter.resolve(&file).is_some());
    fs::remove_file(bin.join("tidy-fmt")).unwrap();
    formatter.clear_cache();
    assert_eq!(formatter.resolve(&file), None);
}
