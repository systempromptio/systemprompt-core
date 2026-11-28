//! Choosing the shell: names, fallbacks and the arguments each family gets.

use std::path::Path;

use systemprompt_engine::tools::{ShellKind, ShellSpec};

fn spec_at(name: &str) -> (tempfile::TempDir, Option<ShellSpec>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::fs::write(&path, b"").unwrap();
    let spec = ShellSpec::resolve(&path);
    (dir, spec)
}

#[test]
fn shell_names_are_recognised_whatever_their_case_or_extension() {
    for (name, kind) in [
        ("PWSH.EXE", ShellKind::Pwsh),
        ("pwsh", ShellKind::Pwsh),
        ("PowerShell.exe", ShellKind::WindowsPowerShell),
        ("Cmd.Exe", ShellKind::Cmd),
    ] {
        assert_eq!(spec_at(name).1.map(|spec| spec.kind), Some(kind), "{name}");
    }
}

#[test]
fn a_configured_shell_that_does_not_resolve_falls_back_to_the_detected_one() {
    let detected = ShellSpec::detect(None);
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(ShellSpec::detect(Some(&dir.path().join("gone"))), detected);
    assert_eq!(
        ShellSpec::detect(Some(Path::new("no-such-shell-anywhere-xyz"))),
        detected
    );
}

#[test]
fn a_configured_shell_that_resolves_wins() {
    let (_dir, spec) = spec_at("pwsh");
    let spec = spec.unwrap();
    assert_eq!(ShellSpec::detect(Some(&spec.path)), spec);
}

#[cfg(unix)]
#[test]
fn a_bare_name_is_looked_up_on_the_search_path() {
    let spec = ShellSpec::resolve(Path::new("sh")).unwrap();
    assert!(spec.path.is_absolute());
    assert_eq!(spec.kind, ShellKind::Posix);
    assert_eq!(
        ShellSpec::resolve(Path::new("no-such-shell-anywhere-xyz")),
        None
    );
}

#[cfg(unix)]
#[test]
#[ignore = "parity ledger: a file that is not executable is accepted as a shell"]
fn a_file_that_is_not_executable_is_not_a_shell() {
    let (_dir, spec) = spec_at("bash");
    assert_eq!(spec, None);
}

#[cfg(unix)]
#[test]
#[ignore = "parity ledger: shells unsuited to running commands are not refused"]
fn a_shell_unsuited_to_scripts_falls_back() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let fish = dir.path().join("fish");
    std::fs::write(&fish, b"#!/bin/sh\n").unwrap();
    std::fs::set_permissions(&fish, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_ne!(ShellSpec::detect(Some(&fish)).path, fish);
}

#[cfg(unix)]
#[tokio::test]
async fn each_shell_family_is_given_its_own_arguments() {
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt as _;
    use systemprompt_engine::tools::{BackgroundJobs, OutputLimits, ShellConfig, ShellTool};
    let project = crate::support::Project::new();
    let fake = project.root.join("pwsh");
    std::fs::write(
        &fake,
        "#!/bin/sh\nfor a in \"$@\"; do echo \"[$a]\"; done\n",
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let spec = ShellSpec::resolve(&fake).unwrap();
    assert_eq!(spec.kind, ShellKind::Pwsh);
    let shell = ShellTool::with_spec(
        spec,
        ShellConfig::default(),
        OutputLimits::default(),
        BackgroundJobs::new(),
    );
    let out = project
        .call(&shell, json!({"command": "Get-Thing"}))
        .await
        .ok();
    assert!(
        out.output
            .starts_with("[-NoLogo]\n[-NoProfile]\n[-NonInteractive]\n[-Command]\n"),
        "{}",
        out.output
    );
    assert!(out.output.contains("Get-Thing]"), "{}", out.output);
    assert_eq!(out.metadata["shell"], "PowerShell 7");
}

#[cfg(windows)]
#[test]
fn bare_windows_shell_names_resolve_to_installed_executables() {
    let cmd = ShellSpec::resolve(Path::new("cmd")).unwrap();
    assert_eq!(cmd.kind, ShellKind::Cmd);
    assert!(cmd.path.is_absolute());
}
