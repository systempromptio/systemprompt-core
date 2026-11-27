use std::path::Path;

use systemprompt_engine::tools::{Dialect, ShellKind, ShellSpec};

fn kind_of(name: &str) -> ShellKind {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, b"").unwrap();
    ShellSpec::resolve(&path).unwrap().kind
}

#[test]
fn kinds_follow_the_file_stem() {
    assert_eq!(kind_of("pwsh.exe"), ShellKind::Pwsh);
    assert_eq!(kind_of("powershell.exe"), ShellKind::WindowsPowerShell);
    assert_eq!(kind_of("CMD.EXE"), ShellKind::Cmd);
    assert_eq!(ShellKind::Cmd.dialect(), Dialect::Cmd);
    assert_eq!(ShellKind::Pwsh.dialect(), Dialect::PowerShell);
}

#[test]
fn missing_paths_do_not_resolve() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(ShellSpec::resolve(&dir.path().join("absent/sh")), None);
}

#[test]
fn detects_a_shell() {
    let spec = ShellSpec::detect(None);
    assert!(!spec.path.as_os_str().is_empty());
    let fallback = ShellSpec::detect(Some(Path::new("definitely-not-a-shell-xyz")));
    assert_eq!(fallback, spec);
}
