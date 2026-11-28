use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;

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

#[cfg(target_os = "linux")]
const PLATFORM_ORDER: [&str; 2] = ["bash", "sh"];
#[cfg(target_os = "macos")]
const PLATFORM_ORDER: [&str; 3] = ["zsh", "bash", "sh"];

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn the_platform_order_picks_the_first_shell_on_the_path() {
    let expected = PLATFORM_ORDER
        .iter()
        .copied()
        .find(|name| on_path(name).is_some())
        .unwrap_or("sh");
    let spec = ShellSpec::detect(None);
    assert_eq!(spec.path.file_name().unwrap(), expected);
    assert_eq!(spec.kind, ShellKind::Posix);
}

#[cfg(unix)]
#[test]
fn a_shell_of_no_named_family_is_posix_on_unix() {
    assert_eq!(kind_of("zsh"), ShellKind::Posix);
    assert_eq!(kind_of("bash"), ShellKind::Posix);
    assert_eq!(kind_of("fish"), ShellKind::Posix);
    assert_eq!(ShellKind::Posix.dialect(), Dialect::Posix);
}

#[cfg(windows)]
#[test]
fn a_shell_of_no_named_family_is_git_bash_on_windows() {
    assert_eq!(kind_of("bash.exe"), ShellKind::GitBash);
    assert_eq!(ShellKind::GitBash.dialect(), Dialect::Posix);
}

#[cfg(windows)]
#[test]
fn windows_prefers_powershell_seven_and_always_finds_a_shell() {
    let spec = ShellSpec::detect(None);
    if ShellSpec::resolve(Path::new("pwsh")).is_some() {
        assert_eq!(spec.kind, ShellKind::Pwsh);
    }
    assert_ne!(spec.kind, ShellKind::Posix);
}

#[test]
fn a_configured_shell_wins_over_the_platform_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cmd.exe");
    std::fs::write(&path, b"").unwrap();
    let spec = ShellSpec::detect(Some(&path));
    assert_eq!((spec.path, spec.kind), (path, ShellKind::Cmd));
}
